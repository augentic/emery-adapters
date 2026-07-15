//! The shared judgment-call shape every adapter operation uses.
//!
//! One leg is one schema-gated `create`: assemble the request from the
//! embedded-prompt `system` and the operation's `user` prompt, offer the
//! adapter's own MCP reference grant, lend the shared workspace, and
//! deserialize the host-validated answer into the leg's typed shape.
//!
//! Source legs run through [`schema_gated`], which brackets the call
//! with a deterministic answer tail inside a bounded repair loop
//! (mirroring the engine's `project::judgment` kernel). Target legs
//! keep the one-shot [`judgment`] helper — build and merge mutate the
//! workspace, so they are never replayed automatically.

use omnia_guest::Model;
use omnia_guest::model::{Format, Message, Reply, Request, Role, SchemaFormat, Tool};
use serde::de::DeserializeOwned;

use crate::seam::{Context, Error};

/// Maximum repair attempts after the first answer.
///
/// A tail failure re-prompts with the findings inlined at most this
/// many times before the leg surfaces the last failure. Matches the
/// engine's `project::judgment::MAX_REPAIRS`.
pub const MAX_REPAIRS: usize = 2;

/// Issue one schema-gated judgment leg and deserialize its answer.
///
/// The request carries `format: schema(...)` under `schema_name` /
/// `schema` (so the host gate validates the reply before the guest sees
/// it), the reference grants from `ctx`, and a workspace lend of the
/// guest's `"."` preopen.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects the request
/// as malformed, and [`Error::Internal`] for other model failures or an
/// answer that does not deserialize into `T`.
pub async fn judgment<P: Model, T: DeserializeOwned>(
    model: &P, ctx: &Context<'_>, system: String, user: String, schema_name: &str, schema: &str,
) -> Result<T, Error> {
    let reply = create(model, ctx, &system, user, schema_name, schema).await?;
    serde_json::from_str(&reply.answer)
        .map_err(|err| Error::Internal(format!("{schema_name} answer did not deserialize: {err}")))
}

/// Issue one schema-gated judgment leg with a bounded repair loop.
///
/// `tail` is the deterministic validation over the raw answer (typed
/// serde parse plus semantic validation). On a tail failure the leg
/// re-prompts with the original request, the rejected answer, and the
/// findings inlined, up to [`MAX_REPAIRS`] times. Only answer-tail
/// failures ([`Error::Internal`] from `tail`) are retried; model,
/// invalid-request, and I/O failures return immediately — the request
/// did not change, so replaying it is pointless.
///
/// # Errors
///
/// The mapped model failure, a non-repairable tail failure, or the last
/// tail failure once the repair budget is exhausted.
pub async fn schema_gated<P, T, F>(
    model: &P, ctx: &Context<'_>, system: String, user: String, schema_name: &str, schema: &str,
    mut tail: F,
) -> Result<T, Error>
where
    P: Model,
    F: FnMut(&str) -> Result<T, Error>,
{
    let mut prompt = user.clone();
    let mut attempt = 0;
    loop {
        let reply = create(model, ctx, &system, prompt, schema_name, schema).await?;
        match tail(&reply.answer) {
            Ok(value) => return Ok(value),
            Err(err @ Error::Internal(_)) if attempt < MAX_REPAIRS => {
                attempt += 1;
                prompt = repair_prompt(&user, &reply.answer, &err);
            }
            Err(err) => return Err(err),
        }
    }
}

/// One `create` call: schema-constrained output, the reference grants
/// from `ctx`, the guest's `"."` preopen lent as the shared workspace.
async fn create<P: Model>(
    model: &P, ctx: &Context<'_>, system: &str, user: String, schema_name: &str, schema: &str,
) -> Result<Reply, Error> {
    model
        .create(
            Request::builder()
                .system(system)
                .messages(vec![Message {
                    role: Role::User,
                    content: user,
                }])
                .format(Format::Schema(
                    SchemaFormat::builder().name(schema_name).schema(schema).build(),
                ))
                .tools(ctx.grants().into_iter().map(Tool::Mcp).collect())
                .lend_workspace(true)
                .build(),
        )
        .await
        .map_err(Error::from)
}

/// Assemble the repair prompt: the original request, the answer that
/// failed the deterministic tail, and the findings to correct.
fn repair_prompt(user: &str, failed_answer: &str, err: &Error) -> String {
    format!(
        "{user}\n\n## Previous answer (failed validation)\n\n{failed_answer}\n\n\
         ## Findings\n\n{err}\n\n\
         Produce a corrected, complete answer that resolves every finding."
    )
}

//! The shared judgment-call shape every adapter operation uses.
//!
//! One leg is one schema-gated `create`: assemble the request from the
//! embedded-prompt `system` and the operation's `user` prompt, offer the
//! adapter's own MCP reference grant, lend the shared workspace, and
//! deserialize the host-validated answer into the leg's typed shape.
//! Flow control around the legs — sub-flow ordering, repair loops,
//! validate-before-visible enforcement — stays adapter-local.

use omnia_guest::Model;
use omnia_guest::model::{Format, Message, Request, Role, SchemaFormat, Tool};
use serde::de::DeserializeOwned;

use crate::seam::{Context, Error};

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
    let reply = model
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
        .await?;
    serde_json::from_str(&reply.answer)
        .map_err(|err| Error::Internal(format!("{schema_name} answer did not deserialize: {err}")))
}

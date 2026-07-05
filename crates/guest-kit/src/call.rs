//! The shared judgment-call shape every adapter operation uses.
//!
//! One leg is one schema-gated `create`: assemble the request from the
//! brief-derived `system` and the operation's `user` prompt, offer the
//! adapter's own MCP reference grant, lend the shared workspace, and
//! deserialize the host-validated answer into the leg's typed shape.
//! Flow control around the legs — sub-flow ordering, repair loops,
//! validate-before-visible enforcement — stays adapter-local.

use serde::de::DeserializeOwned;

use crate::model::{Format, Message, Model, Request, Role, SchemaFormat};
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
        .create(Request {
            model: None,
            system: Some(system),
            messages: vec![Message {
                role: Role::User,
                content: user,
            }],
            format: Format::Schema(SchemaFormat {
                name: schema_name.to_string(),
                schema: schema.to_string(),
            }),
            mcp: ctx.grants(),
            lend_workspace: true,
        })
        .await?;
    serde_json::from_str(&reply.answer)
        .map_err(|err| Error::Internal(format!("{schema_name} answer did not deserialize: {err}")))
}

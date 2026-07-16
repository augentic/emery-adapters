//! Omnia `runtime!` host for the wasm change example.
//!
//! Binds the live cursor backend behind `wasi-model`: command mode drives
//! the deployed `specify:core` component's `wasi:cli/run` export once per
//! verb and exits with its status, while the HTTP trigger serves each
//! adapter guest's MCP reference route in the background for the spawned
//! `cursor-agent`. Run through the root `cargo make change-*` tasks (see
//! [`README.md`](README.md)), or by hand:
//!
//! ```text
//! cargo run -p change-example -- run --config examples/change/omnia.toml -- <specify args>
//! ```
//!
//! Requires `cursor-agent` on `PATH`, authenticated via `CURSOR_API_KEY` or a
//! prior `cursor-agent login`.
//!
//! `SPECIFY_EVAL_MODEL=<model-id>` overrides the model for the run: the
//! driver fills `Request.model` with the id only when the guest left it
//! `None`, letting authors iterate on a fast model. The override is wholly
//! driver-side — it never enters a guest or the WIT contract, and the cursor
//! backend itself stays free of environment configuration. Unset or blank
//! means no override: requests pass through untouched.

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use std::sync::Arc;

        use anyhow::Result;
        use omnia::futures::FutureExt as _;
        use omnia::{Backend, FromEnv};
        use omnia_cursor::Client as Cursor;
        use omnia_wasi_http::{HttpDefault, WasiHttp};
        use omnia_wasi_model::{
            Answer, FutureResult, Request, ToolHost, WasiModel, WasiModelCtx,
        };

        /// Extra completion attempts after a narration-polluted answer.
        ///
        /// cursor-agent's terminal `result` event concatenates every
        /// assistant message in the turn, so a model that narrates before
        /// its final JSON fails the backend's answer gate even when the
        /// closing message is valid. The pollution is per-spawn
        /// stochastic, so one fresh completion is usually enough; anything
        /// deeper belongs upstream in omnia-cursor's output parser.
        const MAX_RETRIES: usize = 2;

        /// Driver-side model decorator around the cursor backend: fills
        /// `Request.model` from `SPECIFY_EVAL_MODEL` when the guest left it
        /// `None`, then delegates. Without an override every request passes
        /// through untouched.
        #[derive(Clone, Debug)]
        struct EvalModel {
            inner: Cursor,
            model: Option<String>,
        }

        impl Backend for EvalModel {
            type ConnectOptions = EvalModelOptions;

            async fn connect_with(options: Self::ConnectOptions) -> Result<Self> {
                Ok(Self { inner: Cursor::connect().await?, model: options.model })
            }
        }

        /// Connection options for [`EvalModel`]: the optional model-id
        /// override, read once from `SPECIFY_EVAL_MODEL`.
        #[derive(Clone, Debug)]
        struct EvalModelOptions {
            model: Option<String>,
        }

        impl FromEnv for EvalModelOptions {
            fn from_env() -> Result<Self> {
                Ok(Self { model: parse_override(std::env::var("SPECIFY_EVAL_MODEL").ok()) })
            }
        }

        /// Appended as the prompt's closing system message: the spawned
        /// agent's terminal result concatenates all of its assistant
        /// messages, so any narration before the final JSON breaks the
        /// answer gate. The instruction suppresses the narration at the
        /// source.
        const OUTPUT_DISCIPLINE: &str = "Critical output discipline: work silently. Never send \
             progress or explanation messages while you work — use tools without commentary. \
             Your one and only message must be the final answer: a single JSON value with no \
             surrounding prose.";

        impl WasiModelCtx for EvalModel {
            fn complete(&self, mut request: Request, tool_host: Arc<dyn ToolHost>) -> FutureResult<Answer> {
                request.model = override_model(request.model, self.model.as_deref());
                request.messages.push(omnia_wasi_model::Message {
                    role: omnia_wasi_model::Role::System,
                    content: OUTPUT_DISCIPLINE.to_owned(),
                });
                let inner = self.inner.clone();
                async move {
                    let mut attempt = 0;
                    loop {
                        match inner.complete(duplicate(&request), Arc::clone(&tool_host)).await {
                            Err(error) if attempt < MAX_RETRIES && narration_polluted(&error) => {
                                attempt += 1;
                                eprintln!(
                                    "retrying completion {attempt}/{MAX_RETRIES}: {error:#}"
                                );
                            }
                            outcome => return outcome,
                        }
                    }
                }
                .boxed()
            }
        }

        /// Whether the failure is the answer-gate rejection a fresh
        /// completion can plausibly clear, as opposed to a connection,
        /// spawn, or timeout failure a replay would only repeat.
        fn narration_polluted(error: &anyhow::Error) -> bool {
            format!("{error:#}").contains("not valid JSON")
        }

        /// Field-by-field rebuild of a wire [`Request`], which derives no
        /// `Clone` because `grants.workspace` may carry a filesystem
        /// resource handle. The host resolves and removes that handle
        /// before any backend sees the request, so the rebuild carries
        /// `workspace: None` by construction.
        fn duplicate(request: &Request) -> Request {
            use omnia_wasi_model::{
                Format, Function, Generation, Grants, Mcp, Schema, Tool,
            };

            Request {
                model: request.model.clone(),
                system: request.system.clone(),
                messages: request
                    .messages
                    .iter()
                    .map(|message| omnia_wasi_model::Message {
                        role: message.role,
                        content: message.content.clone(),
                    })
                    .collect(),
                generation: request.generation.as_ref().map(|generation| Generation {
                    temperature: generation.temperature,
                    top_p: generation.top_p,
                    max_tokens: generation.max_tokens,
                    stop: generation.stop.clone(),
                    seed: generation.seed,
                    effort: generation.effort,
                }),
                format: match &request.format {
                    Format::Text => Format::Text,
                    Format::Json => Format::Json,
                    Format::Schema(schema) => Format::Schema(Schema {
                        name: schema.name.clone(),
                        schema: schema.schema.clone(),
                    }),
                },
                tools: request
                    .tools
                    .iter()
                    .map(|tool| match tool {
                        Tool::Function(function) => Tool::Function(Function {
                            name: function.name.clone(),
                            description: function.description.clone(),
                            parameters: function.parameters.clone(),
                        }),
                        Tool::Mcp(mcp) => Tool::Mcp(Mcp {
                            name: mcp.name.clone(),
                            tools: mcp.tools.clone(),
                            url: mcp.url.clone(),
                        }),
                    })
                    .collect(),
                grants: Grants {
                    references: request.grants.references.clone(),
                    workspace: None,
                    verify: request.grants.verify.clone(),
                },
            }
        }

        /// Normalize the raw override value: unset or blank means none.
        fn parse_override(raw: Option<String>) -> Option<String> {
            raw.filter(|id| !id.trim().is_empty())
        }

        /// Fill the request's model from the driver override only when the
        /// guest left it `None`; a guest-supplied id always wins.
        fn override_model(model: Option<String>, fallback: Option<&str>) -> Option<String> {
            model.or_else(|| fallback.map(str::to_owned))
        }

        omnia::runtime!({
            mode: command,
            hosts: {
                WasiHttp: HttpDefault,
                WasiModel: EvalModel,
            }
        });

        #[cfg(test)]
        mod tests {
            use super::{override_model, parse_override};

            #[test]
            fn fills_none() {
                assert_eq!(override_model(None, Some("fast")), Some("fast".to_owned()));
            }

            #[test]
            fn keeps_guest_id() {
                assert_eq!(
                    override_model(Some("chosen".to_owned()), Some("fast")),
                    Some("chosen".to_owned())
                );
            }

            #[test]
            fn no_override_passes_through() {
                assert_eq!(override_model(None, None), None);
                assert_eq!(override_model(Some("chosen".to_owned()), None), Some("chosen".to_owned()));
            }

            #[test]
            fn blank_env_is_no_override() {
                assert_eq!(parse_override(None), None);
                assert_eq!(parse_override(Some(String::new())), None);
                assert_eq!(parse_override(Some("  ".to_owned())), None);
                assert_eq!(parse_override(Some("fast".to_owned())), Some("fast".to_owned()));
            }
        }
    } else {
        fn main() {}
    }
}

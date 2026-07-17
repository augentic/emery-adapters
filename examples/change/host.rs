//! Omnia `runtime!` host for the wasm change example.
//!
//! Command mode drives `specify:core`; HTTP serves adapter MCP routes for
//! `cursor-agent`. See [`README.md`](README.md) or `cargo make change-*`.

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use std::sync::Arc;

        use anyhow::Result;
        use omnia::{Backend, FromEnv};
        use omnia_cursor::Client as Cursor;
        use omnia_wasi_http::{HttpDefault, WasiHttp};
        use omnia_wasi_model::{
            Answer, FutureResult, Request, ToolHost, WasiModel, WasiModelCtx,
        };

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

        #[derive(Clone, Debug)]
        struct EvalModelOptions {
            model: Option<String>,
        }

        impl FromEnv for EvalModelOptions {
            fn from_env() -> Result<Self> {
                Ok(Self { model: parse_override(std::env::var("SPECIFY_EVAL_MODEL").ok()) })
            }
        }

        // Appended so the cursor backend isolates bare JSON, not narration.
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
                self.inner.complete(request, tool_host)
            }
        }

        fn parse_override(raw: Option<String>) -> Option<String> {
            raw.filter(|id| !id.trim().is_empty())
        }

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

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
        use omnia::{Backend, FromEnv};
        use omnia_cursor::Client as Cursor;
        use omnia_wasi_http::{HttpDefault, WasiHttp};
        use omnia_wasi_model::{
            Answer, FutureResult, Request, ToolHost, WasiModel, WasiModelCtx,
        };

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

        impl WasiModelCtx for EvalModel {
            fn complete(&self, mut request: Request, tool_host: Arc<dyn ToolHost>) -> FutureResult<Answer> {
                request.model = override_model(request.model, self.model.as_deref());
                self.inner.complete(request, tool_host)
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

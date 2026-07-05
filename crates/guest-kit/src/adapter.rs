//! The `wasm32` shim macros: one invocation per adapter-root crate.
//!
//! Every adapter guest is the same shim over its wasm-free core —
//! `wit_bindgen::generate!` bindings for its world, seam-type mapping at
//! the export boundary, the WASI-backed [`Model`](crate::Model) provider,
//! and a `wasi:http` handler serving the core's embedded prose registry
//! as an MCP reference shelf.
//!
//! [`source_adapter!`](crate::source_adapter) and
//! [`target_adapter!`](crate::target_adapter) generate that shim so the
//! eight adapter roots cannot drift.
//!
//! The seam operations are `async func`s, so the exports async-lift and
//! the judgment legs await the async `omnia:model/completion.create`
//! import directly — a sync-lifted export may never block, so there is
//! no sync bridge to hide behind. The MCP grant URL is read from the
//! environment (`SPECIFY_<NAME>_MCP_URL`, the [`mcp_url`](crate::shelf::mcp_url)
//! convention), never hardcoded — absent, judgment legs run without a
//! reference grant.

/// Generate the `wasm32` shim for a source adapter: the
/// `augentic:specify` `source-adapter` world (`survey` / `extract`) over
/// the named core crate, plus the MCP reference shelf.
///
/// Invoke once at the crate root of an adapter-root package (the wit tree
/// resolves at `../../wit` relative to the adapter's manifest):
///
/// ```ignore
/// specify_guest_kit::source_adapter! { name: "intent", core: specify_intent_core }
/// ```
#[macro_export]
macro_rules! source_adapter {
    (name: $name:literal, core: $core:ident $(,)?) => {
        #[allow(
            missing_docs,
            unsafe_code,
            clippy::pedantic,
            clippy::nursery,
            reason = "wit-bindgen generated bindings are not hand-maintained; the generated code cannot carry this workspace's lint posture"
        )]
        mod bindings {
            use super::Adapter;

            wit_bindgen::generate!({
                world: "source-adapter",
                path: "../../wit",
                // The seam operations are `async func`s (judgment legs await
                // the async `omnia:model` import mid-call), so the exports
                // async-lift.
                async: true,
            });

            export!(Adapter);
        }

        use bindings::exports::augentic::specify::source::{
            AdapterId, Authority, Backing, Claim, ClaimKind, Error, Evidence, Guest, Lead,
        };

        $crate::adapter_shared!(name: $name, core: $core);

        impl Guest for Adapter {
            async fn survey(id: AdapterId) -> Result<Vec<Lead>, Error> {
                let url = $crate::shelf::mcp_url($name);
                let ctx = context(&id, url.as_deref());
                $core::operations::survey(&WasiModel, &ctx)
                    .await
                    .map(|leads| leads.into_iter().map(lead_into_wit).collect())
                    .map_err(error_into_wit)
            }

            async fn extract(id: AdapterId, lead: Lead) -> Result<Evidence, Error> {
                let lead = $crate::seam::Lead {
                    lead: lead.lead,
                    synopsis: lead.synopsis,
                    topics: lead.topics,
                };
                let url = $crate::shelf::mcp_url($name);
                let ctx = context(&id, url.as_deref());
                $core::operations::extract(&WasiModel, &ctx, &lead)
                    .await
                    .map(evidence_into_wit)
                    .map_err(error_into_wit)
            }
        }

        fn lead_into_wit(lead: $crate::seam::Lead) -> Lead {
            Lead {
                lead: lead.lead,
                synopsis: lead.synopsis,
                topics: lead.topics,
            }
        }

        fn evidence_into_wit(evidence: $crate::seam::Evidence) -> Evidence {
            Evidence {
                authority: match evidence.authority {
                    $crate::seam::Authority::Intent => Authority::Intent,
                    $crate::seam::Authority::Documentation => Authority::Documentation,
                    $crate::seam::Authority::Behaviour => Authority::Behaviour,
                },
                claims: evidence.claims.into_iter().map(claim_into_wit).collect(),
            }
        }

        fn claim_into_wit(claim: $crate::seam::Claim) -> Claim {
            Claim {
                kind: kind_into_wit(claim.kind),
                id: claim.id,
                path: claim.path,
                synopsis: claim.synopsis,
                backing: claim.backing.map(|backing| match backing {
                    $crate::seam::Backing::Payload(payload) => Backing::Payload(payload),
                    $crate::seam::Backing::Path(path) => Backing::Path(path),
                }),
            }
        }

        const fn kind_into_wit(kind: $crate::seam::ClaimKind) -> ClaimKind {
            match kind {
                $crate::seam::ClaimKind::Intent => ClaimKind::Intent,
                $crate::seam::ClaimKind::Requirement => ClaimKind::Requirement,
                $crate::seam::ClaimKind::Criterion => ClaimKind::Criterion,
                $crate::seam::ClaimKind::Decision => ClaimKind::Decision,
                $crate::seam::ClaimKind::Section => ClaimKind::Section,
                $crate::seam::ClaimKind::Diagram => ClaimKind::Diagram,
                $crate::seam::ClaimKind::Contract => ClaimKind::Contract,
                $crate::seam::ClaimKind::Example => ClaimKind::Example,
                $crate::seam::ClaimKind::Excerpt => ClaimKind::Excerpt,
                $crate::seam::ClaimKind::Type => ClaimKind::Type,
                $crate::seam::ClaimKind::Call => ClaimKind::Call,
                $crate::seam::ClaimKind::Region => ClaimKind::Region,
                $crate::seam::ClaimKind::Container => ClaimKind::Container,
                $crate::seam::ClaimKind::Leaf => ClaimKind::Leaf,
            }
        }
    };
}

/// Generate the `wasm32` shim for a target adapter: the
/// `augentic:specify` `target-adapter` world (`guidance` / `build` /
/// `merge`) over the named core crate, plus the MCP reference shelf.
///
/// Invoke once at the crate root of an adapter-root package (the wit tree
/// resolves at `../../wit` relative to the adapter's manifest):
///
/// ```ignore
/// specify_guest_kit::target_adapter! { name: "omnia", core: specify_omnia_core }
/// ```
#[macro_export]
macro_rules! target_adapter {
    (name: $name:literal, core: $core:ident $(,)?) => {
        #[allow(
            missing_docs,
            unsafe_code,
            clippy::pedantic,
            clippy::nursery,
            reason = "wit-bindgen generated bindings are not hand-maintained; the generated code cannot carry this workspace's lint posture"
        )]
        mod bindings {
            use super::Adapter;

            wit_bindgen::generate!({
                world: "target-adapter",
                path: "../../wit",
                // The seam operations are `async func`s (judgment legs await
                // the async `omnia:model` import mid-call), so the exports
                // async-lift.
                async: true,
            });

            export!(Adapter);
        }

        use bindings::exports::augentic::specify::target::{
            AdapterId, BuildOutput, Changeset, Error, Finding, Guest, Input, Platform, Report,
            Severity, Status, UiSurface, WorkingTree,
        };

        $crate::adapter_shared!(name: $name, core: $core);

        impl Guest for Adapter {
            async fn guidance(_id: AdapterId) -> Result<String, Error> {
                Ok($core::operations::guidance().to_string())
            }

            async fn build(
                id: AdapterId, slice: String, inputs: Vec<Input>, tree: WorkingTree,
            ) -> Result<Report, Error> {
                let inputs: Vec<$crate::seam::Input> =
                    inputs.into_iter().map(input_into_core).collect();
                let tree = tree_into_core(tree);
                let url = $crate::shelf::mcp_url($name);
                let ctx = context(&id, url.as_deref());
                $core::operations::build(&WasiModel, &ctx, &slice, &inputs, &tree)
                    .await
                    .map(report_into_wit)
                    .map_err(error_into_wit)
            }

            async fn merge(
                id: AdapterId, slice: String, delta: Changeset, tree: WorkingTree,
            ) -> Result<Report, Error> {
                let delta = $crate::seam::Changeset {
                    base: delta.base,
                    edits: delta
                        .edits
                        .into_iter()
                        .map(|edit| $crate::seam::Edit {
                            path: edit.path,
                            content: edit.content,
                        })
                        .collect(),
                };
                let tree = tree_into_core(tree);
                let url = $crate::shelf::mcp_url($name);
                let ctx = context(&id, url.as_deref());
                $core::operations::merge(&WasiModel, &ctx, &slice, &delta, &tree)
                    .await
                    .map(report_into_wit)
                    .map_err(error_into_wit)
            }
        }

        fn input_into_core(input: Input) -> $crate::seam::Input {
            match input {
                Input::Proposal(body) => $crate::seam::Input::Proposal(body),
                Input::Design(body) => $crate::seam::Input::Design(body),
                Input::Tasks(body) => $crate::seam::Input::Tasks(body),
                Input::Spec(body) => $crate::seam::Input::Spec(body),
                Input::Other(body) => $crate::seam::Input::Other(body),
            }
        }

        fn tree_into_core(tree: WorkingTree) -> $crate::seam::WorkingTree {
            $crate::seam::WorkingTree {
                base: tree.base,
                subpath: tree.subpath,
            }
        }

        fn report_into_wit(report: $crate::seam::Report) -> Report {
            Report {
                status: match report.status {
                    $crate::seam::Status::Success => Status::Success,
                    $crate::seam::Status::Failure => Status::Failure,
                },
                findings: report
                    .findings
                    .into_iter()
                    .map(|finding| Finding {
                        rule_id: finding.rule_id,
                        severity: match finding.severity {
                            $crate::seam::Severity::Critical => Severity::Critical,
                            $crate::seam::Severity::Important => Severity::Important,
                            $crate::seam::Severity::Suggestion => Severity::Suggestion,
                            $crate::seam::Severity::Optional => Severity::Optional,
                        },
                        detail: finding.detail,
                    })
                    .collect(),
                outputs: report
                    .outputs
                    .into_iter()
                    .map(|output| BuildOutput {
                        platform: match output.platform {
                            $crate::seam::Platform::Core => Platform::Core,
                            $crate::seam::Platform::Ios => Platform::Ios,
                            $crate::seam::Platform::Android => Platform::Android,
                            $crate::seam::Platform::Web => Platform::Web,
                            $crate::seam::Platform::Desktop => Platform::Desktop,
                        },
                        path: output.path,
                    })
                    .collect(),
                ui_surface: report.ui_surface.map(|surface| UiSurface {
                    screens: surface.screens,
                }),
            }
        }
    };
}

/// The axis-independent shim plumbing both adapter macros expand: the
/// export struct, the WASI-backed `Model` provider, the call-scoped
/// context, the seam-error mapping, and the `wasi:http` MCP shelf.
#[doc(hidden)]
#[macro_export]
macro_rules! adapter_shared {
    (name: $name:literal,core: $core:ident) => {
        struct Adapter;

        /// The WASI-backed judgment provider: the `Model` trait's `wasm32`
        /// default method body delegates to the `omnia-wasi-model` bindings.
        struct WasiModel;

        impl $crate::Model for WasiModel {}

        /// The call-scoped context: every guest in the deployment shares the
        /// same `[[mount]]` preopens, so the operation root is the guest's
        /// own `"."`.
        fn context<'a>(adapter_id: &'a str, mcp_url: Option<&'a str>) -> $crate::seam::Context<'a> {
            $crate::seam::Context {
                adapter_id,
                project_root: ::std::path::Path::new("."),
                mcp_url,
            }
        }

        fn error_into_wit(error: $crate::seam::Error) -> Error {
            match error {
                $crate::seam::Error::InvalidRequest(detail) => Error::InvalidRequest(detail),
                $crate::seam::Error::Io(detail) => Error::Io(detail),
                $crate::seam::Error::Internal(detail) => Error::Internal(detail),
            }
        }

        struct HttpGuest;

        wasip3::http::service::export!(HttpGuest);

        impl wasip3::exports::http::handler::Guest for HttpGuest {
            async fn handle(
                request: wasip3::http::types::Request,
            ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
                let shelf = $crate::shelf::Shelf {
                    server_name: concat!("specify-", $name, "-references"),
                    version: env!("CARGO_PKG_VERSION"),
                    docs: $core::registry::docs(),
                };
                omnia_wasi_http::serve(omnia_guest::mcp::router(shelf), request).await
            }
        }
    };
}

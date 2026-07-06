//! The `target-adapter` world bindings, generated once and shared by
//! every target adapter shim.
//!
//! Follows omnia's `wasi-*` guest convention: one `wit_bindgen::generate!`
//! in a library crate with `pub_export_macro`, flat re-exports, and a
//! per-crate [`export!`] invocation in each consumer. A shim implements
//! [`Guest`] for its own type and wires it in with
//! `adapter::target::export!(Adapter with_types_in
//! adapter::target)`.
//!
//! The [`From`] impls below map the generated seam records onto the
//! wasm-free [`crate::seam`] vocabulary at the export boundary, so the
//! shims stay thin delegations to their core crates.

mod generated {
    #![allow(
        missing_docs,
        unsafe_code,
        clippy::pedantic,
        clippy::nursery,
        reason = "wit-bindgen generated bindings are not hand-maintained; the generated code cannot carry this workspace's lint posture"
    )]

    wit_bindgen::generate!({
        world: "target-adapter",
        path: "../../wit",
        // Asyncness follows the WIT declarations: the judgment operations
        // are `async func`s (they await the async `omnia:model` import
        // mid-call) and async-lift; `describe` is a plain `func` (RFC-64:
        // deterministic, effect-free) and sync-lifts — forcing it async
        // would fail component validation at load.
        generate_all,
        pub_export_macro: true,
    });
}

pub use generated::exports::specify::adapter::target::*;
pub use generated::*;

impl From<crate::seam::BuildInput> for BuildInput {
    fn from(input: crate::seam::BuildInput) -> Self {
        Self {
            path: input.path,
            required: input.required,
        }
    }
}

impl From<crate::seam::PlatformsCapability> for PlatformsCapability {
    fn from(capability: crate::seam::PlatformsCapability) -> Self {
        Self {
            required: capability.required,
            allowed: capability.allowed.into_iter().map(Into::into).collect(),
            default: capability.default.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::seam::TargetManifest> for Manifest {
    fn from(manifest: crate::seam::TargetManifest) -> Self {
        Self {
            specify_floor: manifest.specify_floor,
            inputs: manifest.inputs.into_iter().map(Into::into).collect(),
            platforms: manifest.platforms.map(Into::into),
        }
    }
}

impl From<Input> for crate::seam::Input {
    fn from(input: Input) -> Self {
        match input {
            Input::Proposal(body) => Self::Proposal(body),
            Input::Design(body) => Self::Design(body),
            Input::Tasks(body) => Self::Tasks(body),
            Input::Spec(body) => Self::Spec(body),
            Input::Other(body) => Self::Other(body),
        }
    }
}

impl From<WorkingTree> for crate::seam::WorkingTree {
    fn from(tree: WorkingTree) -> Self {
        Self {
            base: tree.base,
            subpath: tree.subpath,
        }
    }
}

impl From<Changeset> for crate::seam::Changeset {
    fn from(delta: Changeset) -> Self {
        Self {
            base: delta.base,
            edits: delta
                .edits
                .into_iter()
                .map(|edit| crate::seam::Edit {
                    path: edit.path,
                    content: edit.content,
                })
                .collect(),
        }
    }
}

impl From<crate::seam::Status> for Status {
    fn from(status: crate::seam::Status) -> Self {
        match status {
            crate::seam::Status::Success => Self::Success,
            crate::seam::Status::Failure => Self::Failure,
        }
    }
}

impl From<crate::seam::Severity> for Severity {
    fn from(severity: crate::seam::Severity) -> Self {
        match severity {
            crate::seam::Severity::Critical => Self::Critical,
            crate::seam::Severity::Important => Self::Important,
            crate::seam::Severity::Suggestion => Self::Suggestion,
            crate::seam::Severity::Optional => Self::Optional,
        }
    }
}

impl From<crate::seam::Finding> for Finding {
    fn from(finding: crate::seam::Finding) -> Self {
        Self {
            rule_id: finding.rule_id,
            severity: finding.severity.into(),
            detail: finding.detail,
        }
    }
}

impl From<crate::seam::Platform> for Platform {
    fn from(platform: crate::seam::Platform) -> Self {
        match platform {
            crate::seam::Platform::Core => Self::Core,
            crate::seam::Platform::Ios => Self::Ios,
            crate::seam::Platform::Android => Self::Android,
            crate::seam::Platform::Web => Self::Web,
            crate::seam::Platform::Desktop => Self::Desktop,
        }
    }
}

impl From<crate::seam::BuildOutput> for BuildOutput {
    fn from(output: crate::seam::BuildOutput) -> Self {
        Self {
            platform: output.platform.into(),
            path: output.path,
        }
    }
}

impl From<crate::seam::UiSurface> for UiSurface {
    fn from(surface: crate::seam::UiSurface) -> Self {
        Self {
            screens: surface.screens,
        }
    }
}

impl From<crate::seam::Report> for Report {
    fn from(report: crate::seam::Report) -> Self {
        Self {
            status: report.status.into(),
            findings: report.findings.into_iter().map(Into::into).collect(),
            outputs: report.outputs.into_iter().map(Into::into).collect(),
            ui_surface: report.ui_surface.map(Into::into),
        }
    }
}

impl From<crate::seam::Error> for Error {
    fn from(error: crate::seam::Error) -> Self {
        match error {
            crate::seam::Error::InvalidRequest(detail) => Self::InvalidRequest(detail),
            crate::seam::Error::Io(detail) => Self::Io(detail),
            crate::seam::Error::Internal(detail) => Self::Internal(detail),
        }
    }
}

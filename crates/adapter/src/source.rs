//! The `source-adapter` world bindings, generated once and shared by
//! every source adapter shim.
//!
//! Follows omnia's `wasi-*` guest convention: one `wit_bindgen::generate!`
//! in a library crate with `pub_export_macro`, flat re-exports, and a
//! per-crate [`export!`] invocation in each consumer. A shim implements
//! [`Guest`] for its own type and wires it in with
//! `adapter::source::export!(Adapter with_types_in
//! adapter::source)`.
//!
//! The [`From`] impls map the generated records onto the [`crate::seam`]
//! vocabulary at the export boundary.

mod generated {
    #![allow(
        missing_docs,
        unsafe_code,
        clippy::pedantic,
        clippy::nursery,
        reason = "wit-bindgen generated bindings are not hand-maintained; the generated code cannot carry this workspace's lint posture"
    )]

    wit_bindgen::generate!({
        world: "source-adapter",
        path: "../../wit",
        // Judgment ops are async; `metadata` is sync.
        generate_all,
        pub_export_macro: true,
    });
}

pub use generated::exports::specify::adapter::source::*;
pub use generated::*;

impl From<crate::seam::SourceMetadata> for AdapterMetadata {
    fn from(metadata: crate::seam::SourceMetadata) -> Self {
        Self {
            specify_floor: metadata.specify_floor,
        }
    }
}

impl From<crate::seam::Lead> for Lead {
    fn from(lead: crate::seam::Lead) -> Self {
        Self {
            lead: lead.lead,
            synopsis: lead.synopsis,
            topics: lead.topics,
        }
    }
}

impl From<Lead> for crate::seam::Lead {
    fn from(lead: Lead) -> Self {
        Self {
            lead: lead.lead,
            synopsis: lead.synopsis,
            topics: lead.topics,
        }
    }
}

impl From<crate::seam::Authority> for Authority {
    fn from(authority: crate::seam::Authority) -> Self {
        match authority {
            crate::seam::Authority::Intent => Self::Intent,
            crate::seam::Authority::Documentation => Self::Documentation,
            crate::seam::Authority::Behaviour => Self::Behaviour,
        }
    }
}

impl From<crate::seam::ClaimKind> for ClaimKind {
    fn from(kind: crate::seam::ClaimKind) -> Self {
        match kind {
            crate::seam::ClaimKind::Intent => Self::Intent,
            crate::seam::ClaimKind::Requirement => Self::Requirement,
            crate::seam::ClaimKind::Criterion => Self::Criterion,
            crate::seam::ClaimKind::Decision => Self::Decision,
            crate::seam::ClaimKind::Section => Self::Section,
            crate::seam::ClaimKind::Diagram => Self::Diagram,
            crate::seam::ClaimKind::Contract => Self::Contract,
            crate::seam::ClaimKind::Example => Self::Example,
            crate::seam::ClaimKind::Excerpt => Self::Excerpt,
            crate::seam::ClaimKind::Type => Self::Type,
            crate::seam::ClaimKind::Call => Self::Call,
            crate::seam::ClaimKind::Region => Self::Region,
            crate::seam::ClaimKind::Container => Self::Container,
            crate::seam::ClaimKind::Leaf => Self::Leaf,
        }
    }
}

impl From<crate::seam::Backing> for Backing {
    fn from(backing: crate::seam::Backing) -> Self {
        match backing {
            crate::seam::Backing::Payload(payload) => Self::Payload(payload),
            crate::seam::Backing::Path(path) => Self::Path(path),
        }
    }
}

impl From<crate::seam::Claim> for Claim {
    fn from(claim: crate::seam::Claim) -> Self {
        Self {
            kind: claim.kind.into(),
            id: claim.id,
            path: claim.path,
            synopsis: claim.synopsis,
            backing: claim.backing.map(Into::into),
        }
    }
}

impl From<crate::seam::Evidence> for Evidence {
    fn from(evidence: crate::seam::Evidence) -> Self {
        Self {
            authority: evidence.authority.into(),
            claims: evidence.claims.into_iter().map(Into::into).collect(),
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

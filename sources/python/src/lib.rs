//! Extracts behavioural claims from Python source.
//!
//! A workspace's production modules — its `.py` files outside tests, stubs,
//! virtual environments, caches, build output, and migrations — are parsed,
//! and the surfaces a caller enters the source through are named in one
//! survey call from what the code says: the manifest (`pyproject.toml` or
//! `setup.cfg`: the package, and the console scripts it installs with what
//! each runs), the bootstrap — the first module a script names, or
//! conventional entry the tree holds, that runs something when loaded, which
//! the adapter names `start` itself — every decorator a package provides on
//! a function or a class, every call outside a handler that hands a function
//! or a class to something a package provides, and what the entry modules
//! export, each at its lines, with the modules laid into the call as far as
//! they fit. The answer names each surface — a route, a command, a task, a
//! consumer, an exported API — at the lines that declare or register it and
//! the modules no surface reaches, and is held to the tree before anything
//! rests on it: an anchor names a module the tree holds, no surface leads
//! with `start`, and every module the facts locate a surface in is reached
//! by a named surface or listed as unreached. From the accepted anchors the
//! code reads the rest: the stem its `requirement` and `criterion` ids lead
//! with — the resource a route spells under its router's prefix or its
//! blueprint's mount, a literal's first word, where the code spells one,
//! else the survey's — the id that tells it from the other surfaces under
//! that stem (a route's verb and path, a handler's name, a method), an
//! exported class's public methods, and the modules it reaches.
//!
//! A tree whose modules fit within the SDK's inline budget is mined in one
//! call over every module, laid into the turn, held to every surface's stem;
//! a larger tree is mined one call per stem, over the modules its surfaces
//! reach, each held to that stem alone — and, where one of those imports a
//! module the tree does not hold or loads one by a computed name, over the
//! rest of the tree after them, so what the import names stays within reach.
//! Each call is told its surfaces, each with its id and what it reaches; the
//! boundaries its modules spell as values of their own — module constants,
//! compiled patterns, `os.environ` reads with whatever the code does to
//! them, settings and dataclass fields with their defaults — the packages
//! they import, the calls they make through those packages, grouped by
//! callee at their sites, the data files they open by path, laid after them
//! as the seam's data, and what could not be followed. Inline input is mined
//! whole, with no survey call.
//!
//! The `type` claims of the source are copied from its declarations — every
//! public class, type alias, and enumeration the seams' modules declare at
//! module level, as its header and member signatures — and joined after the
//! model's answer, which is held to the other kinds; a name led by an
//! underscore, or left out of a declared `__all__`, is none.
//!
//! A workspace with no production module is rejected; one whose survey names
//! no surface is mined under a mechanical cut — one call under the package's
//! or the root directory's name, or one per top-level directory past the
//! budget — and read as a library is. A module the parser cannot read is
//! walked as far as it got and never fails the run.

#![expect(
    clippy::multiple_crate_versions,
    reason = "ruff's `itertools` and the SDK's host-side `prost-derive` pin different minors; \
              neither is this crate's to unify"
)]

#[cfg(target_arch = "wasm32")]
mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, ClaimKind, Context, Error, Evidence, Model, SourceKind};

    use crate::PROSE;
    use crate::survey::{self, Survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Behaviour)
    }

    // An inline value is one seam at once; a workspace's surfaces are named
    // in one turn from what the parser read, and its seams cut from them.
    async fn surveyed<P: Model>(ctx: &Context<'_, P>) -> Result<Survey, Error> {
        match survey::prepare(ctx.input)? {
            survey::Preparation::Value(survey) => Ok(survey),
            survey::Preparation::Workspace(prepared) => {
                let surfaces = survey::model::surfaces(ctx, PROSE, &prepared).await?;
                Ok(survey::seams(&prepared, &surfaces))
            }
        }
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let Survey { seams, types } = surveyed(ctx).await?;
        let mut evidence = emery_sdk::extract(ctx, PROSE, &seams).await?;

        // the declarations are the code's to state: what the model answered
        // as a `type` gives way to what the parser read
        let answered = evidence.claims.len();
        evidence.claims.retain(|claim| claim.kind != ClaimKind::Type);
        let dropped = answered - evidence.claims.len();
        if dropped > 0 {
            emery_sdk::tracing::debug!(
                dropped,
                "type claims the model answered give way to the parsed declarations"
            );
        }
        evidence.claims.extend(types);
        Ok(evidence)
    }
}

/// The prompts and reference documents embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose![
    "../prose/extract.md",
    "../prose/survey.md",
    "../prose/references/examples/README.md",
    "../prose/references/examples/branching-caching.md",
    "../prose/references/examples/outbound-http.md",
    "../prose/references/examples/parallel-execution.md",
];

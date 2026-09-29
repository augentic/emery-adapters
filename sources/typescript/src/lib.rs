//! Extracts behavioural claims from TypeScript and JavaScript source.
//!
//! A workspace's production modules — its `.ts`, `.tsx`, `.js`, and sibling
//! files outside tests, declarations, dependencies, and build output — are
//! parsed, and the surfaces a caller enters the source through are found
//! from the code: the bootstrap — the first entry `package.json` names, or
//! conventional entry the tree holds, that runs something when loaded — each
//! handler registered with a package, each method under a package's
//! decorator, and, in a tree with no bootstrap, each function and class an
//! entry module exports. Each surface carries the stem its `requirement` and
//! `criterion` ids lead with, the id that tells it from the other surfaces
//! under that stem — a handler's name, a route's verb and path, a method —
//! and the modules it reaches.
//!
//! A tree whose modules fit within the SDK's inline budget is mined in one
//! call over every module, laid into the turn, held to every surface's stem;
//! a larger tree is mined one call per stem, over the modules its surfaces
//! reach, each held to that stem alone — and, where one of those imports a
//! module the tree does not hold or loads one by a computed name, over the
//! rest of the tree after them, so what the import names stays within reach.
//! Each call is told its surfaces, each with its id and what it reaches; the
//! boundaries its modules spell as values of their own — literals and
//! expressions over them, patterns, `process.env` reads with whatever the
//! code does to them, definitions handed literals, at module level, in a
//! class, or as a constant-named local — the packages they import, the calls
//! they make through those packages, grouped by callee at their sites, the
//! `.json` files they import, laid after them as the seam's data, and what
//! could not be followed. Inline input is mined whole.
//!
//! The `type` claims of the source are copied from its declarations —
//! every `interface`, `type`, `enum`, and class the seams' modules export,
//! verbatim — and joined after the model's answer, which is held to the other
//! kinds; a declaration a module keeps to itself is none.
//!
//! A workspace with no production module is rejected; one that exposes no
//! surface is mined under a mechanical cut — one call under the package's or
//! the root directory's name, or one per top-level directory past the budget
//! — and read as a library is. A module the parser cannot read is walked as
//! far as it got and never fails the run.

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

    // The surfaces are the parser's, and no turn is spent before the seams'.
    #[cfg(not(feature = "model-survey"))]
    #[expect(clippy::unused_async, reason = "the model arm awaits a turn here; one call site")]
    async fn surveyed<P: Model>(ctx: &Context<'_, P>) -> Result<Survey, Error> {
        survey::survey(ctx.input)
    }

    // The surfaces are the model's, named in one turn from what the parser
    // read; the seams are cut from them as from the parser's own, which the
    // log carries beside them.
    #[cfg(feature = "model-survey")]
    async fn surveyed<P: Model>(ctx: &Context<'_, P>) -> Result<Survey, Error> {
        match survey::prepare(ctx.input)? {
            survey::Preparation::Value(survey) => Ok(survey),
            survey::Preparation::Workspace(prepared) => {
                survey::parsed(&prepared);
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
        let dropped = evidence.claims.extract_if(.., |claim| claim.kind == ClaimKind::Type).count();
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

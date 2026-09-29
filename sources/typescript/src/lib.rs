//! Extracts behavioural claims from TypeScript and JavaScript source.
//!
//! A workspace's production modules — its `.ts`, `.tsx`, `.js`, and sibling
//! files outside tests, declarations, dependencies, and build output — are
//! parsed, and the surfaces a caller enters the source through are found
//! from the code: the bootstrap `package.json` names or a conventional entry
//! holds, each handler registered with a package, each method under a
//! package's decorator, and, in a tree with no bootstrap, each function and
//! class an entry module exports. Each surface carries the stem its
//! `requirement` and `criterion` ids lead with, the id that tells it from
//! the other surfaces under that stem — a handler's name, a route's verb and
//! path, a method — and the modules it reaches.
//!
//! A tree whose modules fit within the SDK's inline budget is mined in one
//! call over every module, laid into the turn, held to every surface's stem;
//! a larger tree is mined one call per stem, over the modules its surfaces
//! reach, each held to that stem alone. Each call is told its surfaces, each
//! with its id and what it reaches; the boundaries its modules spell as
//! values of their own — literals and expressions over them, patterns,
//! `process.env` reads with whatever the code does to them, definitions
//! handed literals, at module level, in a class, or as a constant-named local
//! — the packages they import, and the calls they make through those
//! packages, grouped by callee at their sites. Inline input is mined whole.
//!
//! The `type` claims of the source are copied from its declarations —
//! every `interface`, `type`, `enum`, and class the seams' modules export,
//! verbatim — and joined after the model's answer, which is held to the other
//! kinds; a declaration a module keeps to itself is none.
//!
//! A workspace with no production module, or one that exposes no surface, is
//! rejected. A module the parser cannot read is walked as far as it got and
//! never fails the run.

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

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let Survey { seams, types } = survey::survey(ctx.input)?;
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
    "../prose/references/examples/README.md",
    "../prose/references/examples/branching-caching.md",
    "../prose/references/examples/outbound-http.md",
    "../prose/references/examples/parallel-execution.md",
];

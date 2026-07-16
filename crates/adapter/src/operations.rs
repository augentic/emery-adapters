//! The per-axis operations traits — the single Rust statement of the
//! `specify:adapter` contract an adapter implements.
//!
//! An adapter implements its axis trait on a unit type; every other
//! consumer of the contract derives from it: the wasm export macros
//! (`adapter::source!` / `adapter::target!`, `wasm32` only) wire an
//! implementor into a component's exports, and native harnesses dispatch
//! implementors statically through compile-checked trait bounds.
//!
//! These traits state what an *adapter implements*. They are distinct
//! from the workflow capability traits (`project::seam::Source` /
//! `project::seam::Target`), which state what the *engine calls*:
//! instance-based, `<axis>:<name>`-routed, implemented by providers.
//! Both pairs mirror the same WIT interfaces, so they share the bare
//! names and disambiguate by module path.
//!
//! The methods are associated functions, not `&self` methods: each
//! component contains exactly one adapter implementation and carries no
//! instance state. The traits are deliberately not object-safe; no
//! consumer wants `dyn` dispatch.

use std::future::Future;

use omnia_guest::Model;

use crate::registry::Doc;
use crate::seam::{
    Context, Error, Evidence, Input, Lead, MergePhase, Report, SourceMetadata, TargetMetadata,
    WorkingTree,
};

/// What a source adapter implements: `metadata`, the embedded prose
/// registry, and the `survey` / `extract` judgment operations.
///
/// The judgment operations stay generic over [`Model`], so native tests
/// bind scripted doubles and the wasm shim binds `WasiModel`.
pub trait Source {
    /// The axis-local adapter name, e.g. `"captures"`.
    const NAME: &'static str;

    /// Resolve-time metadata.
    fn metadata() -> SourceMetadata;

    /// The embedded prose registry.
    fn docs() -> &'static [Doc];

    /// Lightly survey the bound source into a lead set.
    fn survey<P: Model>(
        model: &P, ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Vec<Lead>, Error>> + Send;

    /// Thoroughly extract one lead's Evidence.
    fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, lead: &Lead,
    ) -> impl Future<Output = Result<Evidence, Error>> + Send;
}

/// What a target adapter implements: `metadata`, the embedded prose
/// registry, the synthesis-guidance prompt, and the `build` / `merge`
/// judgment operations.
///
/// The judgment operations stay generic over [`Model`], so native tests
/// bind scripted doubles and the wasm shim binds `WasiModel`.
pub trait Target {
    /// The axis-local adapter name, e.g. `"vectis"`.
    const NAME: &'static str;

    /// Resolve-time metadata.
    fn metadata() -> TargetMetadata;

    /// The embedded prose registry.
    fn docs() -> &'static [Doc];

    /// The embedded synthesis-guidance prompt.
    fn guidance() -> &'static str;

    /// Build `slice` against the lent working tree.
    fn build<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
    ) -> impl Future<Output = Result<Report, Error>> + Send;

    /// Run one phased merge gate.
    fn merge<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, phase: MergePhase, tree: &WorkingTree,
    ) -> impl Future<Output = Result<Report, Error>> + Send;
}

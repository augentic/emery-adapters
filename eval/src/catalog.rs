//! Native-only catalog of adapter crates linked into `specify-dev`.
//!
//! Each first-party adapter implements its axis operations trait
//! (`adapter::Source` / `adapter::Target`), so the catalog is a typed
//! table — one [`Entry::source`] / [`Entry::target`] constructor per
//! adapter — and the per-operation dispatch functions the seam provider
//! calls are compile-checked trait calls. Adding an adapter is one
//! entry, one dispatch leg per operation, and its Cargo path
//! dependency.

use std::sync::LazyLock;

use adapter::registry::Doc;
use adapter::seam::{self as aseam, Context};
use adapter::{Source, Target, references};
use captures::Captures;
use contracts::Contracts;
use documentation::Documentation;
use error::Error;
use intent::Intent;
use omnia_guest::Model;
use omnia_target::Omnia;
use project::adapter::metadata::Metadata;
use project::adapter::{Axis, BuildInputDeclaration, PlatformsCapability};
use screenshots::Screenshots;
use typescript::Typescript;
use vectis::Vectis;

/// One Rust adapter crate linked into the native shim.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    axis: Axis,
    name: &'static str,
    server_name: &'static str,
    metadata: fn() -> Metadata,
    docs: fn() -> &'static [Doc],
}

impl Entry {
    /// The catalog entry for one linked source implementor.
    fn source<A: Source>() -> Self {
        Self {
            axis: Axis::Source,
            name: A::NAME,
            server_name: references::server_name(A::NAME),
            metadata: || source_metadata(A::metadata()),
            docs: A::docs,
        }
    }

    /// The catalog entry for one linked target implementor.
    fn target<A: Target>() -> Self {
        Self {
            axis: Axis::Target,
            name: A::NAME,
            server_name: references::server_name(A::NAME),
            metadata: || target_metadata(A::metadata()),
            docs: A::docs,
        }
    }

    /// Adapter axis.
    #[must_use]
    pub const fn axis(self) -> Axis {
        self.axis
    }

    /// Axis-local adapter name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// MCP server name.
    #[must_use]
    pub const fn server_name(self) -> &'static str {
        self.server_name
    }

    /// Routed adapter id (`<axis>:<name>`).
    #[must_use]
    pub fn id(self) -> String {
        format!("{}:{}", self.axis.dir_segment().trim_end_matches('s'), self.name)
    }

    /// Adapter metadata projected onto the workflow shape.
    #[must_use]
    pub fn metadata(self) -> Metadata {
        (self.metadata)()
    }

    /// Embedded prose documents.
    #[must_use]
    pub fn docs(self) -> &'static [Doc] {
        (self.docs)()
    }
}

/// Every adapter linked into the native shim.
#[must_use]
pub fn entries() -> &'static [Entry] {
    static ENTRIES: LazyLock<Vec<Entry>> = LazyLock::new(|| {
        vec![
            Entry::source::<Captures>(),
            Entry::target::<Contracts>(),
            Entry::source::<Documentation>(),
            Entry::source::<Intent>(),
            Entry::target::<Omnia>(),
            Entry::source::<Screenshots>(),
            Entry::source::<Typescript>(),
            Entry::target::<Vectis>(),
        ]
    });
    &ENTRIES
}

/// Whether `id` routes to the linked source implementor `A`.
fn routes_source<A: Source>(id: &str) -> bool {
    id.strip_prefix("source:") == Some(A::NAME)
}

/// Whether `id` routes to the linked target implementor `A`.
fn routes_target<A: Target>(id: &str) -> bool {
    id.strip_prefix("target:") == Some(A::NAME)
}

/// Dispatch `survey` to the linked source adapter behind `id`.
pub(crate) async fn survey<M: Model>(
    model: &M, ctx: &Context<'_>, id: &str,
) -> Result<Vec<aseam::Lead>, aseam::Error> {
    if routes_source::<Captures>(id) {
        return Captures::survey(model, ctx).await;
    }
    if routes_source::<Documentation>(id) {
        return Documentation::survey(model, ctx).await;
    }
    if routes_source::<Intent>(id) {
        return Intent::survey(model, ctx).await;
    }
    if routes_source::<Screenshots>(id) {
        return Screenshots::survey(model, ctx).await;
    }
    if routes_source::<Typescript>(id) {
        return Typescript::survey(model, ctx).await;
    }
    Err(unlinked(id))
}

/// Dispatch `extract` to the linked source adapter behind `id`.
pub(crate) async fn extract<M: Model>(
    model: &M, ctx: &Context<'_>, id: &str, lead: &aseam::Lead,
) -> Result<aseam::Evidence, aseam::Error> {
    if routes_source::<Captures>(id) {
        return Captures::extract(model, ctx, lead).await;
    }
    if routes_source::<Documentation>(id) {
        return Documentation::extract(model, ctx, lead).await;
    }
    if routes_source::<Intent>(id) {
        return Intent::extract(model, ctx, lead).await;
    }
    if routes_source::<Screenshots>(id) {
        return Screenshots::extract(model, ctx, lead).await;
    }
    if routes_source::<Typescript>(id) {
        return Typescript::extract(model, ctx, lead).await;
    }
    Err(unlinked(id))
}

/// Serve the linked target adapter's embedded guidance prompt.
pub(crate) fn guidance(id: &str) -> Result<&'static str, aseam::Error> {
    if routes_target::<Contracts>(id) {
        return Ok(Contracts::guidance());
    }
    if routes_target::<Omnia>(id) {
        return Ok(Omnia::guidance());
    }
    if routes_target::<Vectis>(id) {
        return Ok(Vectis::guidance());
    }
    Err(unlinked(id))
}

/// Dispatch `build` to the linked target adapter behind `id`.
pub(crate) async fn build<M: Model>(
    model: &M, ctx: &Context<'_>, id: &str, slice: &str, inputs: &[aseam::Input],
    tree: &aseam::WorkingTree,
) -> Result<aseam::Report, aseam::Error> {
    if routes_target::<Contracts>(id) {
        return Contracts::build(model, ctx, slice, inputs, tree).await;
    }
    if routes_target::<Omnia>(id) {
        return Omnia::build(model, ctx, slice, inputs, tree).await;
    }
    if routes_target::<Vectis>(id) {
        return Vectis::build(model, ctx, slice, inputs, tree).await;
    }
    Err(unlinked(id))
}

/// Dispatch one `merge` gate to the linked target adapter behind `id`.
pub(crate) async fn merge<M: Model>(
    model: &M, ctx: &Context<'_>, id: &str, slice: &str, phase: aseam::MergePhase,
    tree: &aseam::WorkingTree,
) -> Result<aseam::Report, aseam::Error> {
    if routes_target::<Contracts>(id) {
        return Contracts::merge(model, ctx, slice, phase, tree).await;
    }
    if routes_target::<Omnia>(id) {
        return Omnia::merge(model, ctx, slice, phase, tree).await;
    }
    if routes_target::<Vectis>(id) {
        return Vectis::merge(model, ctx, slice, phase, tree).await;
    }
    Err(unlinked(id))
}

/// A dispatch to an adapter id this shim does not link.
fn unlinked(id: &str) -> aseam::Error {
    aseam::Error::InvalidRequest(format!("adapter `{id}` is not linked into the native shim"))
}

/// Look up a linked adapter by axis and name.
///
/// # Errors
///
/// Returns `adapter-not-found` when the catalog has no matching entry.
pub fn get(axis: Axis, name: &str) -> Result<Entry, Error> {
    entries().iter().copied().find(|entry| entry.axis == axis && entry.name == name).ok_or_else(
        || Error::Diag {
            code: "adapter-not-found",
            detail: format!("adapter `{name}` (axis `{axis}`) is not linked into specify-dev"),
        },
    )
}

fn source_metadata(record: aseam::SourceMetadata) -> Metadata {
    Metadata {
        specify_floor: record.specify_floor,
        inputs: Vec::new(),
        platforms: None,
    }
}

fn target_metadata(record: aseam::TargetMetadata) -> Metadata {
    Metadata {
        specify_floor: record.specify_floor,
        inputs: record
            .inputs
            .into_iter()
            .map(|input| BuildInputDeclaration {
                path: input.path,
                required: input.required,
            })
            .collect(),
        platforms: record.platforms.map(|capability| PlatformsCapability {
            required: capability.required,
            allowed: capability.allowed.into_iter().map(platform).collect(),
            default: capability.default.into_iter().map(platform).collect(),
        }),
    }
}

const fn platform(platform: aseam::Platform) -> project::platform::Platform {
    use project::platform::Platform;
    match platform {
        aseam::Platform::Core => Platform::Core,
        aseam::Platform::Ios => Platform::Ios,
        aseam::Platform::Android => Platform::Android,
        aseam::Platform::Web => Platform::Web,
        aseam::Platform::Desktop => Platform::Desktop,
    }
}

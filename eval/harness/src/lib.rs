//! Reusable native eval-harness core: adapter-agnostic machinery for
//! running the Specify workflow in-process, without a wasm runtime.
//!
//! Consumers declare which adapters are linked through the typed
//! [`catalog::Catalog`] builder over the per-axis operations traits
//! (`adapter::Source` / `adapter::Target`); everything else — the seam
//! [`provider::Provider`], the [`native::Native`] model bridge, the
//! [`model::DevModel`] live backend, [`telemetry`], the [`mcp`]
//! reference shelves, and the [`command`] / [`sandbox`] trial plumbing —
//! is generic over that catalog.

pub mod catalog;
pub mod command;
pub mod env;
pub mod fs;
pub mod mcp;
pub mod model;
pub mod native;
pub mod provider;
pub mod sandbox;
pub mod telemetry;

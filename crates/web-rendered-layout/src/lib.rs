//! Experiment / non-production: the rendered-layout facts of a web page.
//!
//! The facts record what a browser rendered, losslessly, across a range of
//! viewport widths and heights: every element's box as `a*W+b` pieces, every
//! text node's line boxes, computed styles, fonts, and assets. They carry
//! the document nesting and no component grouping.
//!
//! - [`FactsFolder`] is the facts as files, and the one-blob [bundle](FactsFolder::to_bundle)
//!   a source hands the engine.
//! - [`assemble`] builds a folder from a raw browser capture.
//! - [`Facts`] reads a folder and answers what was rendered at a [`Viewport`].
//! - [`compare`] scores a render against an original.
//! - [`draw`] and [`overlay`] render deterministic SVG images.
//! - [`review`] is a comparison with its overlays and findings.
//! - [`outline`] reads the page as regions, containers, and leaves.

mod assemble;
mod compare;
mod draw;
mod error;
mod facts;
mod folder;
mod items;
mod outline;
mod review;
mod strip;

pub use assemble::{MAX_PART_BYTES, assemble};
pub use compare::{
    Comparison, Failure, Heights, KindSummary, PASS_MARK, Point, PointComparison, Summary, compare,
    compare_at,
};
pub use draw::{REVIEW_WIDTHS, draw, overlay};
pub use error::Error;
pub use facts::{Facts, NodeId, Viewport, Width};
pub use folder::{BUNDLE_MEDIA_TYPE, FactsFolder};
pub use items::{Item, ItemKind, items};
pub use outline::{Placement, Role, outline};
pub use review::{Review, review};
pub use strip::without_structure;

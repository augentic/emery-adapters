//! Composition to views.
//!
//! Copied from vx-emit's `view.rs`, whose `SwiftUI`, Jetpack Compose and Dioxus emitters and the
//! helpers they shared did not come across; only the Vue views did.
//!
//! The output is a function of its input, the emitter owns every file it writes, and a difference
//! between the tree and the plan is a defect rather than a preference. A view that drifts loses an
//! interaction guarantee, which a reviewer does not catch in a diff, so views are emitted from the
//! declaration rather than written per screen.

pub mod vue;

//! Consolidated integration binary for `adapter`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once.

#[path = "answers.rs"]
mod answers;
#[path = "call.rs"]
mod call;
#[path = "model.rs"]
mod model;
#[path = "registry.rs"]
mod registry;
#[path = "seam.rs"]
mod seam;

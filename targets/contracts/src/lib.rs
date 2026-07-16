//! The contracts target adapter: [`Contracts`] (the `adapter::Target`
//! implementor carrying the format sub-flows, verify-repair loop, and
//! validate-before-visible enforcement), [`validate`]
//! (baseline-contract validators), and `registry` (embedded prose).
//! The wasm32-only `guest` module is one `adapter::target!` invocation.

mod operations;
mod registry;
pub mod validate;

pub use operations::Contracts;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Contracts);
}

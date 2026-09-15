//! Extracts claims from a TypeScript or JavaScript code tree.

#[cfg(feature = "export")]
emery_sdk::source!(crate::Adapter);

mod operations;
mod registry {
    emery_prose::registry!();
}

pub use operations::Adapter;

//! Extracts claims from a tree of written documentation.

#[cfg(feature = "export")]
emery_sdk::source!(crate::Adapter);

mod operations;
mod registry {
    emery_prose::registry!();
}

pub use operations::Adapter;

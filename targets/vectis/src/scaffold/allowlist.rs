//! Canonical path sets that define a materialized Vectis workspace.
//!
//! Single source of truth shared by [`crate::scaffold::materialize`] and
//! [`crate::verify`] completeness checks.

/// Root files copied verbatim from `$TEMPLATE_DIR` when present.
pub(crate) const ROOT_FILES: &[&str] = &[
    "Makefile",
    "Makefile.toml",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "deny.toml",
    "README.md",
    ".gitignore",
];

/// Cross-cutting root directories copied for every project.
pub(crate) const CROSS_CUTTING_ROOT_DIRS: &[&str] =
    &["shared", "ui-contract", "supply-chain", ".maestro"];

/// Shell root directories copied only when their platform token is declared.
pub(crate) const PLATFORM_ROOT_DIRS: &[&str] = &["iOS", "Android"];

/// Entries that must exist in `$TEMPLATE_DIR` for it to be a current exemplar.
pub(crate) const REQUIRED_TEMPLATE_SHAPE_ENTRIES: &[&str] =
    &["Cargo.toml", "shared", "ui-contract"];

pub(crate) const ALWAYS_REQUIRED_ROOT_FILES: &[&str] = &["Makefile.toml"];

pub(crate) const ALWAYS_REQUIRED_ROOT_DIRS: &[&str] = &["supply-chain"];

/// Canonical-UI / Maestro infra required only when a composition declares UI intent.
pub(crate) const UI_REQUIRED_ROOT_DIRS: &[&str] = &["ui-contract", ".maestro"];

//! Embedded prose registry: every prompt, reference, and rule document
//! this adapter ships, keyed by adapter-relative path with symlinks
//! resolved at build time.
//!
//! The guest's MCP shelf serves it as `doc://`
//! resources; the operation template reads prompt bodies from it for
//! system-prompt assembly.

adapter::embed_registry!();

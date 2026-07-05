//! Embedded prose registry: every brief, reference, and rule document
//! this adapter ships, keyed by adapter-relative path with symlinks
//! resolved at build time.
//!
//! The guest's MCP shelf serves it as `doc://`
//! resources; the operation template reads brief bodies from it for
//! prompt assembly.

specify_guest_kit::embed_registry!();

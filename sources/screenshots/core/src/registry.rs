//! Embedded prose registry: every prompt and reference document this
//! adapter ships, keyed by adapter-relative path with symlinks resolved
//! at build time.
//!
//! The guest's MCP references serves it as `doc://` resources;
//! the operations read prompt bodies from it for prompt assembly.

adapter::embed_registry!();

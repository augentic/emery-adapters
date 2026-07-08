//! Embedded prose registry: every prompt and reference document this
//! adapter ships, keyed by adapter-relative path.
//!
//! Served as `doc://` resources by the guest's MCP references; the
//! operations read prompt bodies from it.

adapter::embed_registry!();

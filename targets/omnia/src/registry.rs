//! Embedded prose registry: every prompt, reference, and rule document
//! this adapter ships, keyed by adapter-relative path.
//!
//! Served as `doc://` resources by the guest's MCP references; the
//! operation template reads prompt bodies from it.

adapter::embed_registry!();

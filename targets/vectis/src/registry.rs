//! Embedded prose registry.
//!
//! Every prompt, reference, and rule document this adapter ships, keyed
//! by adapter-relative path with symlinks resolved at build time.
//! Served as `doc://` MCP resources and read for system-prompt
//! assembly.

adapter::registry!();

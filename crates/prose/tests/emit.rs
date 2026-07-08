//! The prose codegen: tree discovery, ordering, symlink
//! resolution, and failure modes.

use std::fs;
use std::path::Path;

use prose::emit_from;
use tempfile::TempDir;

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join("prose").join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, body).expect("write");
}

fn write_file(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, body).expect("write");
}

fn generate(adapter_root: &Path) -> Result<String, String> {
    let out = TempDir::new().expect("out dir");
    emit_from(adapter_root, out.path())?;
    Ok(fs::read_to_string(out.path().join("registry_docs.rs")).expect("generated file"))
}

#[test]
fn emits_sorted_doc_table() {
    let adapter = TempDir::new().expect("adapter root");
    write(adapter.path(), "references/openapi/verifier.md", "# verifier");
    write(adapter.path(), "prompts/guidance.md", "# guidance");
    write(adapter.path(), "prompts/build.md", "# build");
    write(adapter.path(), "prompts/notes.txt", "not embedded");

    let generated = generate(adapter.path()).expect("emit succeeds");

    let build = generated.find(r#"Doc { path: "prompts/build.md""#).expect("build prompt embedded");
    let guidance =
        generated.find(r#"Doc { path: "prompts/guidance.md""#).expect("guidance prompt embedded");
    let verifier = generated
        .find(r#"Doc { path: "references/openapi/verifier.md""#)
        .expect("nested reference embedded");
    assert!(build < guidance && guidance < verifier, "table is sorted by adapter-relative path");
    assert!(!generated.contains("notes.txt"), "non-markdown files are skipped");
    assert!(generated.contains("include_str!"), "bodies ride as include_str! against disk");
    assert!(generated.contains("pub static DOCS"), "table binds the DOCS static");
}

// The embed set is discovered from disk: any tree under `prose/` — and
// any document directly at the `prose/` root — is embedded without being
// declared anywhere.
#[test]
fn discovers_every_tree() {
    let adapter = TempDir::new().expect("adapter root");
    write(adapter.path(), "prompts/build.md", "# build");
    write(adapter.path(), "rules/CON-001.md", "# rule");
    write(adapter.path(), "overview.md", "# overview");

    let generated = generate(adapter.path()).expect("emit succeeds");

    assert!(generated.contains(r#"Doc { path: "rules/CON-001.md""#), "undeclared tree is embedded");
    assert!(
        generated.contains(r#"Doc { path: "overview.md""#),
        "root-level document is keyed without a tree prefix"
    );
}

#[test]
fn resolves_directory_symlinks_inline() {
    let adapter = TempDir::new().expect("adapter root");
    let shared = TempDir::new().expect("shared tree");
    write_file(shared.path(), "runtime/protocol.md", "# protocol");
    write(adapter.path(), "prompts/build.md", "# build");
    fs::create_dir_all(adapter.path().join("prose/references")).expect("mkdir references");
    std::os::unix::fs::symlink(
        shared.path().join("runtime"),
        adapter.path().join("prose/references/spec-runtime"),
    )
    .expect("symlink");

    let generated = generate(adapter.path()).expect("emit succeeds");

    assert!(
        generated.contains(r#"Doc { path: "references/spec-runtime/protocol.md""#),
        "symlinked document is keyed by its symlink-name path: {generated}"
    );
}

#[test]
fn empty_prose_fails() {
    let adapter = TempDir::new().expect("adapter root");
    fs::create_dir_all(adapter.path().join("prose/prompts")).expect("mkdir");

    let err = generate(adapter.path()).expect_err("no documents is an error");
    assert!(err.contains("no markdown documents"), "error names the failure: {err}");
}

#[test]
fn missing_prose_dir_fails() {
    let adapter = TempDir::new().expect("adapter root");

    let err = generate(adapter.path()).expect_err("no documents is an error");
    assert!(err.contains("no markdown documents"), "error names the failure: {err}");
}

#[test]
fn dangling_symlink_fails() {
    let adapter = TempDir::new().expect("adapter root");
    fs::create_dir_all(adapter.path().join("prose/prompts")).expect("mkdir");
    std::os::unix::fs::symlink(
        adapter.path().join("nowhere"),
        adapter.path().join("prose/prompts/dangling"),
    )
    .expect("symlink");

    let err = generate(adapter.path()).expect_err("dangling symlink is an error");
    assert!(err.contains("dangling symlink"), "error points at the symlink: {err}");
}

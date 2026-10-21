//! The Vue app for the fastapi-login design system, pinned byte for byte.
//!
//! `fixtures/fastapi-login/design-system` is the login page of the full-stack `FastAPI` template
//! (`fastapi/full-stack-fastapi-template`, MIT), extracted by ui-ir's `web-to-ir`
//! (`augentic/ui-ir-experiment`, `experiment/behaviour` at `c5decdf`) from the committed capture
//! `captures/fastapi-template/login`:
//!
//! ```sh
//! web-to-ir extract captures/fastapi-template/login <out>/design-system --screen login
//! ```
//!
//! `composition.json`, `tokens.json`, `assets.json` and the four SVGs are copied as written. The
//! composition carries 11 `TODO(...)` holes, so the golden is emitted with holes allowed.
//!
//! `UI_EMIT_BLESS=1 cargo test -p ui-emit --test golden` rewrites the golden from the current emit.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CASE: &str = "fastapi-login";

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn listed(root: &Path, dir: &Path, into: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).expect("the golden directory reads") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            listed(root, &path, into);
        } else {
            let relative = path.strip_prefix(root).expect("under the root");
            into.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn the_fastapi_login_design_system_emits_the_golden_vue_app() {
    let input = manifest().join("tests/fixtures").join(CASE).join("design-system");
    let golden = manifest().join("tests/golden").join(CASE);
    let emit = ui_emit::vue(&input, true).expect("the design system emits");

    if std::env::var_os("UI_EMIT_BLESS").is_some() {
        let _ = fs::remove_dir_all(&golden);
        for file in &emit.files {
            let path = golden.join(&file.path);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            fs::write(&path, &file.body).expect("the golden writes");
        }
    }

    let emitted: BTreeSet<String> = emit.files.iter().map(|file| file.path.clone()).collect();
    let mut pinned = BTreeSet::new();
    listed(&golden, &golden, &mut pinned);
    assert_eq!(emitted, pinned, "the emit writes exactly the golden's files");
    for file in &emit.files {
        let expected = fs::read_to_string(golden.join(&file.path)).expect("the golden file reads");
        assert!(expected == file.body, "{} differs from the golden", file.path);
    }

    let assets: Vec<&str> = emit.assets.values().map(String::as_str).collect();
    assert_eq!(
        assets,
        [
            "assets/e3c1bb4ea8-fastapi-logo-light.svg",
            "assets/svg-1.svg",
            "assets/svg-2.svg",
            "assets/svg-3.svg"
        ]
    );
    assert!(emit.unserved.is_empty(), "{:?}", emit.unserved);
}

#[test]
fn holes_stop_the_emit_unless_allowed() {
    let input = manifest().join("tests/fixtures").join(CASE).join("design-system");
    let error = ui_emit::vue(&input, false).expect_err("the fixture has holes");
    assert!(error.to_string().contains("still has holes"), "{error}");
}

#[test]
fn an_emit_written_twice_leaves_the_workspace_clean() {
    let input = manifest().join("tests/fixtures").join(CASE).join("design-system");
    let emit = ui_emit::vue(&input, true).expect("the design system emits");
    let app = tempfile::tempdir().expect("a temporary directory");
    let pruned = [Path::new(ui_emit::VIEWS)];

    let first = ui_emit::write(app.path(), &emit.files, &pruned).expect("the first write");
    assert_eq!(first.written.len(), emit.files.len());
    let drift = ui_emit::drift(app.path(), &emit.files, &pruned).expect("the check");
    assert!(drift.is_clean(), "{drift:?}");
}

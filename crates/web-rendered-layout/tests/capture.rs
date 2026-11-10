//! Holding a raw capture to the source files it was taken of.

use std::fs;
use std::path::PathBuf;

use web_rendered_layout::stamped;

// sha256("a")
const A: &str = "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";

fn tree(name: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("capture").join(name);
    if root.exists() {
        fs::remove_dir_all(&root).expect("an earlier tree is removed");
    }
    let (capture, sources) = (root.join("capture"), root.join("src"));
    fs::create_dir_all(sources.join("components")).expect("the sources are made");
    fs::create_dir_all(&capture).expect("the capture is made");
    fs::write(sources.join("components/Card.vue"), "a").expect("a source is written");
    let meta =
        serde_json::json!({ "source": { "root": "src", "files": { "components/Card.vue": A } } });
    fs::write(capture.join("meta.json"), meta.to_string()).expect("the meta is written");
    (capture, sources)
}

#[test]
fn unchanged_sources_hold() {
    let (capture, sources) = tree("unchanged");
    stamped(&capture, &sources).expect("the capture is of the sources as they stand");
}

#[test]
fn changed_and_new_sources_are_named() {
    let (capture, sources) = tree("changed");
    fs::write(sources.join("components/Card.vue"), "b").expect("the source is changed");
    fs::write(sources.join("Page.vue"), "a").expect("a source is added");

    let refused = stamped(&capture, &sources).expect_err("the sources moved on").to_string();
    assert!(refused.contains("`components/Card.vue` changed"), "{refused}");
    assert!(refused.contains("`Page.vue` is new"), "{refused}");
}

#[test]
fn unstamped_capture_is_refused() {
    let (capture, sources) = tree("unstamped");
    fs::write(capture.join("meta.json"), "{}").expect("the meta is replaced");

    let refused = stamped(&capture, &sources).expect_err("no stamp").to_string();
    assert!(refused.contains("carries no source stamp"), "{refused}");
}

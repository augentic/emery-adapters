//! The facts as files: the bundle a source hands the engine, and the facts a refactor reads.

mod support;

use support::{BLUE, RED, page};
use web_rendered_layout::{Facts, FactsFolder, without_structure};

#[test]
fn bundle_round_trip() {
    let mut folder = page(&[RED, BLUE]);
    folder.insert("assets/dot.png", vec![0x89, b'P', b'N', b'G', 0xff, 0x00]).expect("a safe path");
    let bundle = folder.to_bundle();
    let back = FactsFolder::from_bundle(&bundle).expect("a bundle reads back");

    assert_eq!(bundle, back.to_bundle());
    assert_eq!(back.get("assets/dot.png"), Some(&[0x89, b'P', b'N', b'G', 0xff, 0x00][..]));
    assert_eq!(back.paths().collect::<Vec<_>>(), ["assets/dot.png", "index.json", "parts/n0.json"]);
    let text = String::from_utf8(bundle).expect("a bundle is JSON");
    assert!(text.starts_with(r#"{"files":{"assets/dot.png":{"base64":"iVBOR/8A"}"#), "{text}");
}

#[test]
fn bundle_refuses_other_formats() {
    let refused = FactsFolder::from_bundle(br#"{"format":"other/1","files":{}}"#)
        .expect_err("another format is refused");

    assert!(refused.to_string().contains("other/1"), "{refused}");
}

#[test]
fn folder_refuses_escaping_paths() {
    let mut folder = FactsFolder::default();
    for path in ["../index.json", "/etc/hosts", "parts/../../x", ""] {
        assert!(folder.insert(path, Vec::new()).is_err(), "{path:?} is refused");
    }
}

#[test]
fn without_structure_keeps_geometry() {
    let mut folder = page(&[RED, BLUE]);
    folder.insert("assets/dot.png", vec![1, 2, 3]).expect("a safe path");
    let stripped = without_structure(&folder).expect("the facts strip");
    let index = stripped.json("index.json").expect("an index");
    let nodes = stripped.json("parts/n0.json").expect("a part");

    for key in ["root", "containers", "ax", "ax_files", "ax_h"] {
        assert!(index.get(key).is_none(), "{key} is dropped");
    }
    assert_eq!(index["parts"], serde_json::json!([{ "file": "parts/n0.json" }]));
    for key in ["tag", "path", "children", "parent"] {
        assert!(nodes["n1"].get(key).is_none(), "{key} is dropped from an element");
    }
    assert_eq!(nodes["t0"]["parent"], "n1");
    assert!(nodes["t0"].get("children").is_none());
    assert_eq!(nodes["n1"]["box"], folder.json("parts/n0.json").expect("a part")["n1"]["box"]);
    assert_eq!(stripped.get("assets/dot.png"), Some(&[1, 2, 3][..]));
    Facts::read(&stripped).expect("stripped facts read");
}

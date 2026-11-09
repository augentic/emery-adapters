use serde_json::{Map, Value, json};

use crate::folder::json_line;
use crate::{Error, FactsFolder};

const KEPT_ATTRIBUTES: [&str; 2] = ["href", "alt"];
const INDEX_STRUCTURE: [&str; 5] = ["ax", "ax_files", "ax_h", "containers", "root"];

/// The facts in `folder` without the document structure: what a refactor reads.
///
/// Every node loses its tag, CSS path, and children, and an element its
/// parent; attributes other than `href` and `alt` go, and so do the index's
/// accessibility trees, containers, and root. Boxes, text lines, styles,
/// fonts, and assets stay.
///
/// # Errors
///
/// When the index or a part it lists is missing or not JSON.
pub fn without_structure(folder: &FactsFolder) -> Result<FactsFolder, Error> {
    let mut index = folder.json("index.json")?;
    let parts: Vec<String> = index["parts"]
        .as_array()
        .ok_or_else(|| Error::new("`index.json` lists no `parts`"))?
        .iter()
        .filter_map(|part| part["file"].as_str().map(str::to_owned))
        .collect();
    let held = index.as_object_mut().ok_or_else(|| Error::new("`index.json` is not an object"))?;
    for key in INDEX_STRUCTURE {
        held.remove(key);
    }
    held.insert("parts".to_owned(), parts.iter().map(|file| json!({ "file": file })).collect());

    let mut out = FactsFolder::default();
    out.insert("index.json", json_line(&index))?;
    for file in &parts {
        let Value::Object(nodes) = folder.json(file)? else {
            return Err(Error::new(format!("`{file}` is not a JSON object of nodes")));
        };
        let stripped: Map<String, Value> =
            nodes.into_iter().map(|(id, node)| (id, strip_node(node))).collect();
        out.insert(file, json_line(&Value::Object(stripped)))?;
    }
    for path in folder.paths().filter(|path| path.starts_with("assets/")) {
        out.insert(path, folder.get(path).unwrap_or_default().to_vec())?;
    }
    Ok(out)
}

fn strip_node(node: Value) -> Value {
    let Value::Object(mut node) = node else {
        return node;
    };
    let text = node.get("tag").and_then(Value::as_str) == Some("#text");
    node.remove("tag");
    node.remove("path");
    node.remove("children");
    if !text {
        node.remove("parent");
    }
    if let Some(Value::Object(attributes)) = node.remove("attrs") {
        let kept: Map<String, Value> = attributes
            .into_iter()
            .filter(|(key, _)| KEPT_ATTRIBUTES.contains(&key.as_str()))
            .collect();
        if !kept.is_empty() {
            node.insert("attrs".to_owned(), Value::Object(kept));
        }
    }
    Value::Object(node)
}

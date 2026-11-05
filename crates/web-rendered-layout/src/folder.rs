use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value, json};

use crate::Error;

/// The media type of a facts [bundle](FactsFolder::to_bundle).
pub const BUNDLE_MEDIA_TYPE: &str = "application/vnd.emery.web-rendered-layout+json";

const BUNDLE_FORMAT: &str = "web-rendered-layout/1";

/// The facts as files: `index.json`, `parts/<id>.json`, `assets/`, and `ax/`.
///
/// Paths are relative, `/`-separated, and never leave the folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactsFolder {
    files: BTreeMap<String, Vec<u8>>,
}

impl FactsFolder {
    /// Reads every regular file beneath `dir`.
    ///
    /// # Errors
    ///
    /// When `dir` or a file beneath it cannot be read, or a name is not UTF-8.
    pub fn read(dir: &Path) -> Result<Self, Error> {
        let mut folder = Self::default();
        walk(dir, "", &mut folder)?;
        if folder.files.is_empty() {
            return Err(Error::new(format!("{} holds no files", dir.display())));
        }
        Ok(folder)
    }

    /// Writes every file beneath `dir`, creating directories as needed.
    ///
    /// # Errors
    ///
    /// When a directory or file cannot be written.
    pub fn write(&self, dir: &Path) -> Result<(), Error> {
        for (path, body) in &self.files {
            let to = dir.join(path);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| Error::new(format!("creating {}: {err}", parent.display())))?;
            }
            fs::write(&to, body)
                .map_err(|err| Error::new(format!("writing {}: {err}", to.display())))?;
        }
        Ok(())
    }

    /// The file at `path`, if the folder holds it.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// The JSON file at `path`.
    ///
    /// # Errors
    ///
    /// When the folder lacks `path` or it is not JSON.
    pub fn json(&self, path: &str) -> Result<Value, Error> {
        let body = self.get(path).ok_or_else(|| Error::new(format!("the facts lack `{path}`")))?;
        serde_json::from_slice(body)
            .map_err(|err| Error::new(format!("`{path}` is not JSON: {err}")))
    }

    /// Puts `body` at `path`, replacing what stood there.
    ///
    /// # Errors
    ///
    /// When `path` is not a relative path inside the folder.
    pub fn insert(&mut self, path: &str, body: Vec<u8>) -> Result<(), Error> {
        inside(path)?;
        self.files.insert(path.to_owned(), body);
        Ok(())
    }

    /// The paths the folder holds, in order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// The folder as one deterministic JSON document of media type [`BUNDLE_MEDIA_TYPE`].
    ///
    /// A UTF-8 file is held as `text`, any other as `base64`; the same
    /// folder always yields the same bytes.
    #[must_use]
    pub fn to_bundle(&self) -> Vec<u8> {
        let files: Map<String, Value> = self
            .files
            .iter()
            .map(|(path, body)| {
                let held = std::str::from_utf8(body).map_or_else(
                    |_| json!({ "base64": STANDARD.encode(body) }),
                    |text| json!({ "text": text }),
                );
                (path.clone(), held)
            })
            .collect();
        let mut bytes = json!({ "format": BUNDLE_FORMAT, "files": files }).to_string().into_bytes();
        bytes.push(b'\n');
        bytes
    }

    /// Reads a bundle [`to_bundle`](Self::to_bundle) wrote.
    ///
    /// # Errors
    ///
    /// When `bundle` is not a bundle of this format, or a path leaves the folder.
    pub fn from_bundle(bundle: &[u8]) -> Result<Self, Error> {
        let document: Value = serde_json::from_slice(bundle)
            .map_err(|err| Error::new(format!("the layout bundle is not JSON: {err}")))?;
        if document["format"] != BUNDLE_FORMAT {
            return Err(Error::new(format!(
                "the layout bundle's format is {}, not `{BUNDLE_FORMAT}`",
                document["format"]
            )));
        }
        let files = document["files"]
            .as_object()
            .ok_or_else(|| Error::new("the layout bundle has no `files` object"))?;
        let mut folder = Self::default();
        for (path, held) in files {
            let body = match (held["text"].as_str(), held["base64"].as_str()) {
                (Some(text), None) => text.as_bytes().to_vec(),
                (None, Some(encoded)) => STANDARD.decode(encoded).map_err(|err| {
                    Error::new(format!("the layout bundle's `{path}` is not base64: {err}"))
                })?,
                _ => {
                    return Err(Error::new(format!(
                        "the layout bundle's `{path}` holds neither `text` nor `base64` alone"
                    )));
                }
            };
            folder.insert(path, body)?;
        }
        Ok(folder)
    }
}

fn walk(dir: &Path, prefix: &str, folder: &mut FactsFolder) -> Result<(), Error> {
    let entries =
        fs::read_dir(dir).map_err(|err| Error::new(format!("reading {}: {err}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|err| Error::new(format!("reading {}: {err}", dir.display())))?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| Error::new(format!("a name in {} is not UTF-8", dir.display())))?;
        let path = format!("{prefix}{name}");
        let kind = entry
            .file_type()
            .map_err(|err| Error::new(format!("reading {}: {err}", entry.path().display())))?;
        if kind.is_dir() {
            walk(&entry.path(), &format!("{path}/"), folder)?;
        } else if kind.is_file() {
            let body = fs::read(entry.path())
                .map_err(|err| Error::new(format!("reading {}: {err}", entry.path().display())))?;
            folder.files.insert(path, body);
        }
    }
    Ok(())
}

fn inside(path: &str) -> Result<(), Error> {
    let normal = !path.is_empty()
        && !path.contains('\\')
        && Path::new(path).components().all(|part| matches!(part, Component::Normal(_)));
    if normal {
        Ok(())
    } else {
        Err(Error::new(format!("`{path}` is not a relative path inside the facts")))
    }
}

//! Holds a raw capture to the source files it says it was taken of.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::Error;

/// The browser capture `web-layout-capture capture` runs, for a tree that captures its own render.
///
/// It is a Node script over Playwright, run as
/// `node capture.mjs --url URL --out DIR [--screenshots DIR] [--stamp DIR]`;
/// [`assemble`](crate::assemble) builds facts from what it writes.
pub const CAPTURE_SCRIPT: &str = include_str!("../../../tools/web-layout-capture/capture.mjs");

/// Checks that the raw capture in `capture` was taken of the files under `sources` as they stand.
///
/// The capture's `meta.json` carries the sha256 of every file under the
/// directory its `--stamp` named. Every file must still hash the same, and no
/// file may have been added since.
///
/// # Errors
///
/// When the capture carries no stamp, or a file changed, was added, or is gone, each named
/// by its path below `sources`.
pub fn stamped(capture: &Path, sources: &Path) -> Result<(), Error> {
    let meta = fs::read(capture.join("meta.json")).map_err(|err| {
        Error::new(format!("reading {}: {err}", capture.join("meta.json").display()))
    })?;
    let meta: Value = serde_json::from_slice(&meta)
        .map_err(|err| Error::new(format!("{}/meta.json is not JSON: {err}", capture.display())))?;
    let Some(stamp) = meta["source"]["files"].as_object() else {
        return Err(Error::new("the capture carries no source stamp; capture with `--stamp`"));
    };
    let mut now = BTreeMap::new();
    hash_tree(sources, "", &mut now)?;

    let mut differences = Vec::new();
    for (path, digest) in stamp {
        match now.get(path) {
            None => differences.push(format!("`{path}` is gone")),
            Some(current) if Some(current.as_str()) != digest.as_str() => {
                differences.push(format!("`{path}` changed"));
            }
            Some(_) => {}
        }
    }
    differences.extend(
        now.keys().filter(|path| !stamp.contains_key(*path)).map(|path| format!("`{path}` is new")),
    );
    if differences.is_empty() {
        return Ok(());
    }
    Err(Error::new(format!(
        "the capture is not of the sources as they stand: {}; capture again",
        differences.join(", ")
    )))
}

fn hash_tree(dir: &Path, prefix: &str, hashes: &mut BTreeMap<String, String>) -> Result<(), Error> {
    let entries =
        fs::read_dir(dir).map_err(|err| Error::new(format!("reading {}: {err}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|err| Error::new(format!("reading {}: {err}", dir.display())))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        let kind = entry
            .file_type()
            .map_err(|err| Error::new(format!("reading {}: {err}", entry.path().display())))?;
        if kind.is_dir() {
            hash_tree(&entry.path(), &format!("{path}/"), hashes)?;
        } else if kind.is_file() {
            let body = fs::read(entry.path())
                .map_err(|err| Error::new(format!("reading {}: {err}", entry.path().display())))?;
            hashes.insert(path, hex(&Sha256::digest(&body)));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

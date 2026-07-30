//! Core verify-completion digest stamp.
//!
//! Agents write `shared/.vectis/verify.ok` only after the final post-review
//! core verify-repair pass. The mid-build verify-repair loop in the core
//! leg must not write this stamp. The report gate treats a missing or stale
//! stamp as blocking when the core tree is present.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::shell::shell_present;

/// Adapter-owned stamp written after a green final core verify.
pub const CORE_VERIFY_STAMP: &str = "shared/.vectis/verify.ok";

/// Emit findings when the core tree is present but its digest stamp is
/// missing, unreadable, or does not match the current `shared/src/**/*.rs`
/// tree.
#[must_use]
pub fn core_stamp_findings(project_root: &Path) -> Vec<Value> {
    if !shell_present(project_root, "core") {
        return Vec::new();
    }

    // Core presence already requires `shared/src/app.rs`. `Ok(None)` or
    // `Err` means the digest could not be attested — fail closed rather
    // than skipping the stamp check.
    let Ok(Some(expected)) = core_src_digest(project_root) else {
        return vec![error_finding(
            "core-verify-digest-unreadable",
            "could not compute the `shared/src/**/*.rs` digest; ensure every \
             `*.rs` file under `shared/src` is readable (skipping `generated/`), \
             then re-run the final core verify-repair pass and rewrite \
             `shared/.vectis/verify.ok`",
        )];
    };

    let stamp_path = project_root.join(CORE_VERIFY_STAMP);
    if !stamp_path.is_file() {
        return vec![error_finding(
            "core-verify-stamp-missing",
            format!(
                "`{CORE_VERIFY_STAMP}` not found; run the final core verify-repair pass \
                 (fmt / check / clippy / test), then write this stamp with the digest of \
                 `shared/src/**/*.rs`"
            ),
        )];
    }

    let Ok(raw) = fs::read_to_string(&stamp_path) else {
        return vec![error_finding(
            "core-verify-stamp-stale",
            format!(
                "`{CORE_VERIFY_STAMP}` is unreadable; re-run the final core verify-repair \
                 pass and rewrite the stamp"
            ),
        )];
    };
    let actual = raw.trim();
    if actual != expected {
        return vec![error_finding(
            "core-verify-stamp-stale",
            format!(
                "`{CORE_VERIFY_STAMP}` digest does not match the current `shared/src/**/*.rs` \
                 tree; re-run the final core verify-repair pass and refresh the stamp"
            ),
        )];
    }

    Vec::new()
}

/// Stable digest of `shared/src/**/*.rs` (sorted relative paths + per-file
/// content hashes), formatted as `sha256:<hex>`.
///
/// Returns `Ok(None)` when `shared/src` is absent.
///
/// # Errors
///
/// Returns `Err` when the tree is present but cannot be fully enumerated or
/// read — a partial digest must never be attested.
pub fn core_src_digest(project_root: &Path) -> Result<Option<String>, String> {
    let src_root = project_root.join("shared/src");
    if !src_root.is_dir() {
        return Ok(None);
    }

    let mut files = Vec::new();
    collect_rs_files(&src_root, &src_root, &mut files)?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));

    let mut canonical = String::new();
    for file in &files {
        let bytes = fs::read(&file.path)
            .map_err(|err| format!("read shared/src/{}: {err}", file.relative))?;
        let file_digest = hex_digest(&bytes);
        canonical.push_str(&file.relative);
        canonical.push('\n');
        canonical.push_str(&file_digest);
        canonical.push('\n');
    }

    Ok(Some(format!("sha256:{}", hex_digest(canonical.as_bytes()))))
}

struct RsFile {
    relative: String,
    path: PathBuf,
}

fn collect_rs_files(dir: &Path, src_root: &Path, out: &mut Vec<RsFile>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("read_dir {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir {}: {err}", dir.display()))?;
        let path = entry.path();
        let file_type =
            entry.file_type().map_err(|err| format!("file_type {}: {err}", path.display()))?;
        if file_type.is_dir() {
            if path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("generated")) {
                continue;
            }
            collect_rs_files(&path, src_root, out)?;
        } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rs")) {
            let relative =
                path.strip_prefix(src_root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            out.push(RsFile { relative, path });
        }
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn error_finding(id: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "severity": "error",
        "source": "deterministic",
        "message": message.into(),
    })
}

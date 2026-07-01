//! Inline lint-suppression scan for `vectis verify --mode verify`.
//!
//! Rejects agent-authored compiler / linter suppressions in core Rust and
//! platform shell sources. Crate-level workspace lints and `generated/`
//! trees are out of scope.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::shell::shell_present;

/// Diagnostic id emitted for every suppression hit.
pub const FINDING_ID: &str = "lint-suppression-forbidden";

/// Collect findings for forbidden inline suppressions under agent-authored trees.
#[must_use]
pub fn suppression_scan_findings(project_root: &Path, platforms: &[String]) -> Vec<Value> {
    let mut findings = Vec::new();

    let shared_src = project_root.join("shared/src");
    if shared_src.is_dir() {
        scan_tree(&shared_src, "rs", true, &mut |path, line_no, matched| {
            findings.push(suppression_finding(
                project_root,
                path,
                line_no,
                format!(
                    "inline Rust lint suppression `{matched}` is forbidden in `shared/src` — fix the underlying structure (see hard-rules-core rule 10)"
                ),
            ));
        });
    }

    if platforms.iter().any(|p| p == "ios") && shell_present(project_root, "ios") {
        let ios_root = project_root.join("iOS");
        scan_tree(&ios_root, "swift", true, &mut |path, line_no, matched| {
            findings.push(suppression_finding(
                project_root,
                path,
                line_no,
                format!(
                    "Swift lint/format suppression `{matched}` is forbidden in agent-authored iOS sources — fix the underlying structure (see hard-rules-ios and ios/write.md)"
                ),
            ));
        });
    }

    if platforms.iter().any(|p| p == "android") && shell_present(project_root, "android") {
        for subtree in ["Android/app/src", "Android/shared/src"] {
            let dir = project_root.join(subtree);
            if !dir.is_dir() {
                continue;
            }
            scan_tree(&dir, "kt", true, &mut |path, line_no, matched| {
                findings.push(suppression_finding(
                    project_root,
                    path,
                    line_no,
                    format!(
                        "Kotlin lint suppression `{matched}` is forbidden in agent-authored Android sources — fix the underlying structure (see hard-rules-android and android/write.md)"
                    ),
                ));
            });
        }
    }

    findings
}

fn suppression_finding(
    project_root: &Path, path: &Path, line_no: usize, message: impl AsRef<str>,
) -> Value {
    let relative =
        path.strip_prefix(project_root).unwrap_or(path).to_string_lossy().replace('\\', "/");
    json!({
        "id": FINDING_ID,
        "severity": "error",
        "source": "deterministic",
        "path": relative,
        "line": line_no,
        "message": message.as_ref(),
    })
}

fn scan_tree(
    root: &Path, extension: &str, exclude_generated: bool,
    on_hit: &mut dyn FnMut(&Path, usize, &str),
) {
    let mut files = Vec::new();
    collect_files(root, extension, exclude_generated, &mut files);
    for path in files {
        scan_file(&path, extension, on_hit);
    }
}

fn collect_files(dir: &Path, extension: &str, exclude_generated: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if exclude_generated
                && path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("generated"))
            {
                continue;
            }
            collect_files(&path, extension, exclude_generated, out);
        } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension)) {
            out.push(path);
        }
    }
}

fn scan_file(path: &Path, extension: &str, on_hit: &mut dyn FnMut(&Path, usize, &str)) {
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };
    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if should_skip_comment_line(extension, trimmed) {
            continue;
        }
        for token in suppression_tokens_on_line(extension, line, trimmed) {
            on_hit(path, line_no + 1, token);
        }
    }
}

fn should_skip_comment_line(extension: &str, trimmed: &str) -> bool {
    match extension {
        "rs" | "kt" => {
            trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*")
        }
        _ => false,
    }
}

fn suppression_tokens_on_line(extension: &str, line: &str, trimmed: &str) -> Vec<&'static str> {
    match extension {
        "rs" => rust_suppression_token(trimmed).into_iter().collect(),
        _ => patterns_for_extension(extension)
            .iter()
            .copied()
            .filter_map(|pattern| line_matches_suppression(extension, line, trimmed, pattern))
            .collect(),
    }
}

fn line_matches_suppression(
    extension: &str, line: &str, trimmed: &str, pattern: &'static str,
) -> Option<&'static str> {
    match extension {
        "rs" => rust_suppression_token(trimmed),
        "kt" => trimmed.starts_with(pattern).then_some(pattern),
        "swift" => swift_suppression_in_comment(line, pattern),
        _ => None,
    }
}

fn rust_suppression_token(trimmed: &str) -> Option<&'static str> {
    let rest = trimmed.strip_prefix('#')?;
    let attr = rest.strip_prefix('!').unwrap_or(rest);
    if attr.starts_with("[allow(") {
        Some(if rest.starts_with('!') { "#![allow(" } else { "#[allow(" })
    } else if attr.starts_with("[expect(") {
        Some(if rest.starts_with('!') { "#![expect(" } else { "#[expect(" })
    } else {
        None
    }
}

fn swift_suppression_in_comment(line: &str, pattern: &'static str) -> Option<&'static str> {
    let pos = line.find(pattern)?;
    (line[..pos].contains("//") || line.trim_start().starts_with("//")).then_some(pattern)
}

fn patterns_for_extension(extension: &str) -> &'static [&'static str] {
    match extension {
        "rs" => &["#[allow(", "#[expect("],
        "swift" => &["swiftlint:disable", "swift-format-ignore"],
        "kt" => &["@Suppress(", "@file:Suppress"],
        _ => &[],
    }
}

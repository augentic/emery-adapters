//! Canonical UI bindings: detect hardcoded UI contract copy and raw test tags in shell/core.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::shell::shell_present;

/// Finding id for hardcoded UI copy that should use generated bindings.
const LITERAL_FINDING_ID: &str = "canonical-ui-literal-hardcoded";

/// Finding id for raw test-tag strings instead of `MaestroTestIds.*`.
const TEST_ID_FINDING_ID: &str = "canonical-test-id-raw";

/// Finding id when Android uses `testTag` without `testTagsAsResourceId` on a root.
const TEST_TAGS_RESOURCE_ID_FINDING_ID: &str = "canonical-test-tag-resource-id";

/// Emit findings when shell/core sources hard-code UI contract copy or raw test tags.
#[must_use]
pub fn ui_literals_findings(project_root: &Path, platforms: &[String]) -> Vec<Value> {
    let strings_yaml = project_root.join("ui-contract/ui-strings.yaml");
    if !strings_yaml.is_file() {
        return Vec::new();
    }

    let mut contract_values = load_yaml_map_values(&strings_yaml, "strings");
    contract_values
        .extend(load_yaml_map_values(&project_root.join("ui-contract/ui-errors.yaml"), "errors"));
    contract_values.sort_by_key(|b| std::cmp::Reverse(b.len()));
    contract_values.dedup();

    let mut findings = Vec::new();

    if platforms.iter().any(|p| p == "ios") && shell_present(project_root, "ios") {
        scan_shell_tree(
            &project_root.join("iOS"),
            "swift",
            project_root,
            &contract_values,
            &mut findings,
        );
    }

    if platforms.iter().any(|p| p == "android") && shell_present(project_root, "android") {
        for subtree in ["Android/app/src", "Android/shared/src"] {
            let dir = project_root.join(subtree);
            if dir.is_dir() {
                scan_shell_tree(&dir, "kt", project_root, &contract_values, &mut findings);
            }
        }
    }

    let shared_src = project_root.join("shared/src");
    if shared_src.is_dir() {
        scan_rust_tree(&shared_src, project_root, &contract_values, &mut findings);
    }

    findings.extend(android_test_tag_resource_id_findings(project_root, platforms));

    // A quoted contract value on a `Text("…")` line is flagged by both the API
    // scan and the generic contract-value scan; keep one finding per (id, path, line).
    let mut seen = BTreeSet::new();
    findings.retain(|finding| {
        let key = (
            finding["id"].as_str().unwrap_or_default().to_owned(),
            finding["path"].as_str().unwrap_or_default().to_owned(),
            finding["line"].as_u64().unwrap_or_default(),
        );
        seen.insert(key)
    });

    findings
}

fn load_yaml_map_values(path: &Path, section: &str) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    // Same shape exemplar codegen reads (`BTreeMap<section, BTreeMap<key, value>>`).
    // Hand-rolled line scans miss quoted values, nested maps, and block scalars.
    let Ok(document) =
        serde_saphyr::from_str::<BTreeMap<String, BTreeMap<String, String>>>(&content)
    else {
        return Vec::new();
    };

    let Some(entries) = document.get(section) else {
        return Vec::new();
    };

    entries.values().filter(|val| !val.is_empty() && !val.starts_with('#')).cloned().collect()
}

fn scan_shell_tree(
    root: &Path, extension: &str, project_root: &Path, contract_values: &[String],
    findings: &mut Vec<Value>,
) {
    let mut files = Vec::new();
    collect_shell_files(root, extension, &mut files);
    for path in files {
        scan_shell_file(&path, project_root, contract_values, findings);
    }
}

fn collect_shell_files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "generated" | "build" | ".build") {
                continue;
            }
            collect_shell_files(&path, extension, out);
        } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension)) {
            out.push(path);
        }
    }
}

fn scan_shell_file(
    path: &Path, project_root: &Path, contract_values: &[String], findings: &mut Vec<Value>,
) {
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    for (line_no, line) in source.lines().enumerate() {
        if is_allowed_shell_line(line) {
            continue;
        }

        if let Some(val) = extract_quoted_after_key(line, "contentDescription") {
            push_literal_finding(
                findings,
                project_root,
                path,
                line_no + 1,
                &format!("use UiStrings.* for contentDescription (got \"{val}\")"),
            );
        }
        if let Some(val) = extract_quoted_after_key(line, "accessibilityLabel") {
            push_literal_finding(
                findings,
                project_root,
                path,
                line_no + 1,
                &format!("use UiStrings.* for accessibilityLabel (got \"{val}\")"),
            );
        }

        for api in [
            "Text(",
            "Text(text = ",
            "Button(",
            "Label(",
            "navigationTitle(",
            "navigationBarTitle(",
        ] {
            if let Some(val) = extract_after_quoted(line, api)
                && should_report_string_literal(val)
            {
                push_literal_finding(
                    findings,
                    project_root,
                    path,
                    line_no + 1,
                    &format!("use UiStrings.* for {} (got \"{val}\")", api.trim_end_matches('(')),
                );
            }
        }

        push_contract_value_findings(
            findings,
            project_root,
            path,
            line_no + 1,
            line,
            contract_values,
            |val| {
                format!(
                    "hardcoded UI contract value \"{val}\" — add/use UiStrings or ui_strings key; run `cargo make generate-bindings`"
                )
            },
        );

        if !line.contains("MaestroTestIds.") {
            if let Some(val) = extract_after_quoted(line, "testTag(") {
                push_test_id_finding(
                    findings,
                    project_root,
                    path,
                    line_no + 1,
                    &format!("use MaestroTestIds.* instead of testTag(\"{val}\")"),
                );
            }
            if let Some(val) = extract_after_quoted(line, "accessibilityIdentifier(") {
                push_test_id_finding(
                    findings,
                    project_root,
                    path,
                    line_no + 1,
                    &format!("use MaestroTestIds.* instead of accessibilityIdentifier(\"{val}\")"),
                );
            }
        }
    }
}

fn scan_rust_tree(
    root: &Path, project_root: &Path, contract_values: &[String], findings: &mut Vec<Value>,
) {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    for path in files {
        scan_rust_file(&path, project_root, contract_values, findings);
    }
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "codegen") {
                continue;
            }
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let rel = path.to_string_lossy();
            if rel.contains("/bin/codegen/")
                || rel.ends_with("ui_strings.rs")
                || rel.ends_with("ui_errors.rs")
                || rel.ends_with("seed_data.rs")
            {
                continue;
            }
            out.push(path);
        }
    }
}

fn scan_rust_file(
    path: &Path, project_root: &Path, contract_values: &[String], findings: &mut Vec<Value>,
) {
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    for (line_no, line) in source.lines().enumerate() {
        if line.contains("ui_strings::")
            || line.contains("ui_errors::")
            || line.contains("include_str!")
        {
            continue;
        }
        push_contract_value_findings(
            findings,
            project_root,
            path,
            line_no + 1,
            line,
            contract_values,
            |val| {
                format!(
                    "hardcoded UI contract value \"{val}\" in core — use ui_strings:: / ui_errors::*"
                )
            },
        );
    }
}

fn is_allowed_shell_line(line: &str) -> bool {
    line.contains("UiStrings.")
        || line.contains("UiErrors.")
        || line.contains("stringResource(")
        || line.contains("LocalizedStringKey")
        || line.contains("import ")
        || (line.contains("//") && !line.contains("Text("))
}

fn extract_after_quoted<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.split_once(prefix)?.1;
    quoted_from_start(rest.trim_start())
}

/// `key`, optional whitespace, `=` or `(`, optional whitespace, `"value"`.
fn extract_quoted_after_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let idx = line.find(key)?;
    let mut rest = line[idx + key.len()..].trim_start();
    if rest.starts_with('(') || rest.starts_with('=') {
        rest = rest[1..].trim_start();
    } else {
        return None;
    }
    quoted_from_start(rest)
}

fn quoted_from_start(rest: &str) -> Option<&str> {
    if !rest.starts_with('"') {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find('"')?;
    Some(&inner[..end])
}

fn should_report_string_literal(value: &str) -> bool {
    value.len() >= 3 && !value.contains('\\') && !value.contains('$')
}

fn push_literal_finding(
    findings: &mut Vec<Value>, project_root: &Path, path: &Path, line: usize, message: &str,
) {
    findings.push(finding(project_root, path, line, LITERAL_FINDING_ID, message));
}

fn push_test_id_finding(
    findings: &mut Vec<Value>, project_root: &Path, path: &Path, line: usize, message: &str,
) {
    findings.push(finding(project_root, path, line, TEST_ID_FINDING_ID, message));
}

/// Emit a hardcoded-contract-value finding for each ui-contract value quoted on `line`.
fn push_contract_value_findings(
    findings: &mut Vec<Value>, project_root: &Path, path: &Path, line_no: usize, line: &str,
    contract_values: &[String], message_for: impl Fn(&str) -> String,
) {
    for val in contract_values {
        if line.contains(&format!("\"{val}\"")) {
            push_literal_finding(findings, project_root, path, line_no, &message_for(val));
        }
    }
}

fn android_test_tag_resource_id_findings(project_root: &Path, platforms: &[String]) -> Vec<Value> {
    if !platforms.iter().any(|p| p == "android") || !shell_present(project_root, "android") {
        return Vec::new();
    }
    if !project_root.join("ui-contract/ui-strings.yaml").is_file() {
        return Vec::new();
    }

    let app_src = project_root.join("Android/app/src");
    if !app_src.is_dir() {
        return Vec::new();
    }

    let mut uses_test_tag = false;
    let mut has_resource_id_flag = false;
    scan_android_maestro_signals(&app_src, &mut uses_test_tag, &mut has_resource_id_flag);

    if uses_test_tag && !has_resource_id_flag {
        return vec![json!({
            "id": TEST_TAGS_RESOURCE_ID_FINDING_ID,
            "severity": "error",
            "source": "deterministic",
            "path": "Android/app/src",
            "message": "Compose shell uses Modifier.testTag but no root semantics { testTagsAsResourceId = true } — Maestro id: selectors will not resolve; enable on the root Surface in ContentView",
        })];
    }

    Vec::new()
}

fn scan_android_maestro_signals(dir: &Path, uses_test_tag: &mut bool, has_resource_id: &mut bool) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "generated" | "build") {
                continue;
            }
            scan_android_maestro_signals(&path, uses_test_tag, has_resource_id);
            continue;
        }
        if !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("kt")) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        if source.contains(".testTag(") || source.contains("testTag(") {
            *uses_test_tag = true;
        }
        if source.contains("testTagsAsResourceId") {
            *has_resource_id = true;
        }
    }
}

fn finding(project_root: &Path, path: &Path, line: usize, id: &str, message: &str) -> Value {
    let relative =
        path.strip_prefix(project_root).unwrap_or(path).to_string_lossy().replace('\\', "/");
    json!({
        "id": id,
        "severity": "error",
        "source": "deterministic",
        "path": relative,
        "line": line,
        "message": message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_allowed_shell_line_accepts_generated_bindings() {
        assert!(is_allowed_shell_line("Text(UiStrings.SPLASH_TITLE)"));
        assert!(!is_allowed_shell_line("Text(\"Task\")"));
    }

    #[test]
    fn should_report_string_literal_filters_short_and_special() {
        assert!(should_report_string_literal("Task"));
        assert!(!should_report_string_literal("ok"));
        assert!(!should_report_string_literal("a$b"));
    }

    #[test]
    fn extract_quoted_literals_from_shell_lines() {
        assert_eq!(
            extract_quoted_after_key(
                "contentDescription = \"Splash illustration\"",
                "contentDescription"
            ),
            Some("Splash illustration")
        );
        assert_eq!(extract_after_quoted("Text(\"Task\")", "Text("), Some("Task"));
        assert_eq!(quoted_from_start("\"Task\")"), Some("Task"));
    }

    #[test]
    fn load_yaml_map_values_reads_quoted_and_plain() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ui-strings.yaml");
        fs::write(&path, "strings:\n  PLAIN: Hello\n  QUOTED: \"Get Organised!\"\n  EMPTY: \"\"\n")
            .unwrap();
        let values = load_yaml_map_values(&path, "strings");
        assert!(values.contains(&"Hello".to_string()));
        assert!(values.contains(&"Get Organised!".to_string()));
        assert!(!values.iter().any(String::is_empty));
    }
}

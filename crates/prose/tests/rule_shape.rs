//! Rule-shape validation over every codex rule this repository authors:
//! the shared `codex/rules/universal/` pack plus each adapter overlay
//! (`{sources,targets}/<name>/prose/rules/`). Structural checks only —
//! required frontmatter fields, the severity enum, id grammar, the
//! `## Rule` body heading, id uniqueness across every tree, and
//! namespace ownership. The canonical JSON Schema in `augentic/specify`
//! (`schemas/rules/rule.schema.json`) re-validates the same shape at
//! every `specify rules export`; this test is the authoring-time gate
//! in the repo that owns the rules.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The closed severity enum from `schemas/rules/rule.schema.json`.
const SEVERITIES: &[&str] = &["critical", "important", "suggestion", "optional"];

/// Frontmatter fields every rule must carry.
const REQUIRED_FIELDS: &[&str] = &["id", "title", "severity", "trigger"];

/// Frontmatter fields retired with the lint engine; their presence is
/// drift back toward deleted machinery.
const RETIRED_FIELDS: &[&str] = &["rule_hints", "lint_mode"];

/// Target-adapter namespace ownership: `targets/<name>/prose/rules/`
/// may only mint ids under the owner's prefixes. Extend this map when
/// a new target adapter grows an overlay.
const TARGET_OWNERS: &[(&str, &[&str])] =
    &[("omnia", &["OMNIA", "RUST", "SEC"]), ("contracts", &["IFACE"]), ("vectis", &["VECTIS"])];

/// Every source-adapter overlay shares the single `SRC` namespace.
const SOURCE_PREFIXES: &[&str] = &["SRC"];

/// Prefixes the shared universal pack owns.
const UNIVERSAL_PREFIXES: &[&str] = &["UNI"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/prose sits two levels below the repo root")
        .to_path_buf()
}

/// A rule tree to validate: the directory plus the prefixes it owns.
struct RuleTree {
    dir: PathBuf,
    prefixes: &'static [&'static str],
}

/// Discover every rule tree in the checkout: the universal pack plus
/// each `{sources,targets}/<name>/prose/rules/` overlay that exists.
fn discover_trees(root: &Path, findings: &mut Vec<String>) -> Vec<RuleTree> {
    let mut trees = Vec::new();
    let universal = root.join("codex/rules/universal");
    if universal.is_dir() {
        trees.push(RuleTree {
            dir: universal,
            prefixes: UNIVERSAL_PREFIXES,
        });
    } else {
        findings.push("codex/rules/universal/ is missing".to_owned());
    }

    for (axis, prefixes_for) in [("targets", None), ("sources", Some(SOURCE_PREFIXES))] {
        let axis_dir = root.join(axis);
        let Ok(entries) = fs::read_dir(&axis_dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy().into_owned();
            let rules_dir = entry.path().join("prose/rules");
            if !rules_dir.is_dir() {
                continue;
            }
            let prefixes = prefixes_for.or_else(|| {
                TARGET_OWNERS.iter().find(|(owner, _)| *owner == name).map(|(_, p)| *p)
            });
            match prefixes {
                Some(prefixes) => trees.push(RuleTree {
                    dir: rules_dir,
                    prefixes,
                }),
                None => findings.push(format!(
                    "{axis}/{name}/prose/rules/ exists but owns no namespace: add the adapter \
                     to TARGET_OWNERS in crates/prose/tests/rule_shape.rs"
                )),
            }
        }
    }
    trees
}

/// Every markdown rule file in `dir`, recursively. `README.md`
/// (case-insensitive) is an index page, never a rule.
fn rule_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("read rule directory {}: {err}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(rule_files(&path));
            continue;
        }
        let is_markdown = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
        let is_readme = path
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case("readme"));
        if is_markdown && !is_readme {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Split a rule document into its top-level frontmatter map and body.
/// Only unindented `key:` lines register as fields; nested block content
/// (lists, sub-maps) rides under the preceding key.
fn parse_frontmatter(content: &str) -> Option<(BTreeMap<String, String>, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let (frontmatter, tail) = rest.split_at(end);
    let body = tail.strip_prefix("\n---").expect("split at the delimiter");

    let mut fields = BTreeMap::new();
    for line in frontmatter.lines() {
        if line.starts_with([' ', '\t', '-', '#']) {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.entry(key.trim().to_owned()).or_insert_with(|| value.trim().to_owned());
        }
    }
    Some((fields, body))
}

/// `PREFIX-NNN`: an owned prefix, a hyphen, exactly three digits.
fn id_matches(id: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| {
        id.strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|digits| digits.len() == 3 && digits.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Validate every rule tree under `root`, returning human-readable
/// findings (empty means the corpus is clean).
fn check_rules(root: &Path) -> Vec<String> {
    let mut findings = Vec::new();
    let mut seen_ids: BTreeMap<String, String> = BTreeMap::new();

    for tree in discover_trees(root, &mut findings) {
        for path in rule_files(&tree.dir) {
            let rel = path.strip_prefix(root).unwrap_or(&path).display().to_string();
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read rule file {rel}: {err}"));

            let Some((fields, body)) = parse_frontmatter(&content) else {
                findings.push(format!("{rel}: missing `---` YAML frontmatter block"));
                continue;
            };

            for field in REQUIRED_FIELDS {
                if fields.get(*field).is_none_or(String::is_empty) {
                    findings.push(format!("{rel}: missing required frontmatter field `{field}`"));
                }
            }
            for field in RETIRED_FIELDS {
                if fields.contains_key(*field) {
                    findings.push(format!(
                        "{rel}: carries retired frontmatter field `{field}` — rules are \
                         agent-readable prose; deterministic hint machinery is deleted"
                    ));
                }
            }
            if let Some(severity) = fields.get("severity")
                && !severity.is_empty()
                && !SEVERITIES.contains(&severity.as_str())
            {
                findings.push(format!("{rel}: severity `{severity}` is not one of {SEVERITIES:?}"));
            }
            if let Some(id) = fields.get("id")
                && !id.is_empty()
            {
                if !id_matches(id, tree.prefixes) {
                    findings.push(format!(
                        "{rel}: id `{id}` violates namespace ownership — this tree owns \
                         {:?} (shape `PREFIX-NNN`)",
                        tree.prefixes
                    ));
                }
                if let Some(previous) = seen_ids.insert(id.clone(), rel.clone()) {
                    findings.push(format!(
                        "{rel}: duplicate rule id `{id}` (also declared by {previous})"
                    ));
                }
            }
            if !body.lines().any(|line| line.trim_end() == "## Rule") {
                findings.push(format!("{rel}: body is missing the required `## Rule` heading"));
            }
        }
    }
    findings
}

/// The whole authored rule corpus is structurally clean.
#[test]
fn corpus_is_clean() {
    let findings = check_rules(&repo_root());
    assert!(findings.is_empty(), "rule-shape findings:\n{}", findings.join("\n"));
}

/// Each check fires on a known-bad fixture, so a silent pass cannot hide
/// a broken walker or parser.
#[test]
fn fires_on_known_bad_fixture() {
    let root = tempfile::TempDir::new().expect("fixture root");
    let write = |rel: &str, body: &str| {
        let path = root.path().join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, body).expect("write fixture");
    };

    write(
        "codex/rules/universal/bad-severity.md",
        "---\nid: UNI-001\ntitle: Bad severity\nseverity: catastrophic\ntrigger: t\n---\n\n## Rule\n\nx\n",
    );
    write(
        "codex/rules/universal/duplicate-id.md",
        "---\nid: UNI-001\ntitle: Duplicate id\nseverity: important\ntrigger: t\n---\n\n## Rule\n\nx\n",
    );
    write(
        "codex/rules/universal/retired-hints.md",
        "---\nid: UNI-002\ntitle: Retired hints\nseverity: important\ntrigger: t\nrule_hints:\n  - kind: regex\n---\n\n## Rule\n\nx\n",
    );
    write(
        "codex/rules/universal/missing-heading.md",
        "---\nid: UNI-003\ntitle: No heading\nseverity: important\ntrigger: t\n---\n\nprose only\n",
    );
    write("codex/rules/universal/no-frontmatter.md", "# not a rule\n");
    write(
        "targets/vectis/prose/rules/wrong-namespace.md",
        "---\nid: OMNIA-001\ntitle: Wrong owner\nseverity: important\ntrigger: t\n---\n\n## Rule\n\nx\n",
    );
    write(
        "targets/mystery/prose/rules/unmapped.md",
        "---\nid: MYS-001\ntitle: Unmapped owner\nseverity: important\ntrigger: t\n---\n\n## Rule\n\nx\n",
    );

    let findings = check_rules(root.path());
    let assert_fires = |needle: &str| {
        assert!(
            findings.iter().any(|f| f.contains(needle)),
            "expected a finding containing `{needle}`; got:\n{}",
            findings.join("\n")
        );
    };
    assert_fires("severity `catastrophic`");
    assert_fires("duplicate rule id `UNI-001`");
    assert_fires("retired frontmatter field `rule_hints`");
    assert_fires("missing the required `## Rule` heading");
    assert_fires("missing `---` YAML frontmatter block");
    assert_fires("violates namespace ownership");
    assert_fires("owns no namespace");
}

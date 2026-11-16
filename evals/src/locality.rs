//! Measures crate-level locality, touch-point adherence, scope precision, amend
//! fidelity, round-trip size, and worker `git` shell invocations across an
//! evolution chain.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};
use std::path::Path;
use std::{fs, io};

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use serde_json::Value;
use syn::visit::{self, Visit};
use syn::{Item, ItemUse, Macro, MetaList, UseTree};

use crate::build::strings;
use crate::card::short;
use crate::evolve::{CrateAction, ExpectedAmend, ExpectedScope};
use crate::stage::{git, git_file};

const EXCLUDED_CRATES: [&str; 1] = ["common"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TouchKind {
    Use,
    Item(String),
    Dependency(String),
}

impl Display for TouchKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Use => f.write_str("use"),
            Self::Item(name) => f.write_str(name),
            Self::Dependency(package) => write!(f, "dependency {package}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchPoint {
    pub file: String,
    pub kind: TouchKind,
    pub line: usize,
}

impl TouchPoint {
    fn admits_item(points: &[Self], head: &ItemHead) -> bool {
        match head {
            ItemHead::Use => !points.is_empty(),
            ItemHead::Named(name) => points
                .iter()
                .any(|point| matches!(&point.kind, TouchKind::Item(item) if item == name)),
        }
    }

    fn admits_dependency(points: &[Self], package: &str) -> bool {
        points
            .iter()
            .any(|point| matches!(&point.kind, TouchKind::Dependency(name) if name == package))
    }
}

impl Display for TouchPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "`{}:{}` (`{}`)", self.file, self.line, self.kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateSize {
    pub files: usize,
    pub loc: usize,
    pub items: usize,
    pub tests: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateSnapshot {
    pub dir: String,
    pub package: String,
    pub ident: String,
    pub tree_hash: String,
    pub size: CrateSize,
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSnapshot {
    pub ref_name: String,
    pub commit: String,
    pub crates: BTreeMap<String, CrateSnapshot>,
    pub root_cargo_toml: Option<String>,
    pub root_lib_rs: Option<String>,
    pub topic_rs: Option<String>,
}

impl TreeSnapshot {
    pub fn read(repo: &Path, ref_name: &str) -> io::Result<Self> {
        let commit = git(repo, &["rev-parse", ref_name])?;
        let crates_tree = git_lines(repo, &["ls-tree", &commit, "crates/"])?;
        let mut crates = BTreeMap::new();
        for entry in crates_tree {
            // `<mode> <type> <hash>\tcrates/<dir>`
            let Some((meta, path)) = entry.split_once('\t') else { continue };
            let Some(dir) = path.strip_prefix("crates/") else { continue };
            if EXCLUDED_CRATES.contains(&dir) {
                continue;
            }
            let mut parts = meta.split_whitespace();
            let (_mode, kind, hash) = (parts.next(), parts.next(), parts.next());
            if kind != Some("tree") {
                continue;
            }
            let Some(tree_hash) = hash.map(str::to_owned) else { continue };
            let files = read_crate_files(repo, &commit, dir)?;
            let package = files
                .get(&format!("crates/{dir}/Cargo.toml"))
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .and_then(manifest_package_name)
                .unwrap_or_else(|| dir.to_owned());
            let ident = package.replace('-', "_");
            let size = measure_crate(&files);
            crates.insert(
                dir.to_owned(),
                CrateSnapshot {
                    dir: dir.to_owned(),
                    package,
                    ident,
                    tree_hash,
                    size,
                    files,
                },
            );
        }

        let root_cargo_toml = git_utf8(repo, &commit, "Cargo.toml")?;
        let root_lib_rs = git_utf8(repo, &commit, "src/lib.rs")?;
        let topic_rs = git_utf8(repo, &commit, "crates/common/src/topic.rs")?;

        Ok(Self {
            ref_name: ref_name.to_owned(),
            commit,
            crates,
            root_cargo_toml,
            root_lib_rs,
            topic_rs,
        })
    }

    // Touch points in crates outside `targeted_dirs` whose `[dependencies]` or
    // top-level Rust items mention one of `targeted_dirs`' packages.
    pub fn touch_points(&self, targeted_dirs: &BTreeSet<String>) -> Vec<TouchPoint> {
        if targeted_dirs.is_empty() {
            return Vec::new();
        }
        let mut packages = BTreeSet::new();
        let mut idents = BTreeSet::new();
        for dir in targeted_dirs {
            if let Some(krate) = self.crates.get(dir) {
                packages.insert(krate.package.clone());
                idents.insert(krate.ident.clone());
            } else {
                packages.insert(dir.clone());
                idents.insert(dir.replace('-', "_"));
            }
        }

        let mut points = Vec::new();
        for (dir, krate) in &self.crates {
            if targeted_dirs.contains(dir) {
                continue;
            }
            let manifest_path = format!("crates/{dir}/Cargo.toml");
            if let Some(bytes) = krate.files.get(&manifest_path)
                && let Ok(text) = std::str::from_utf8(bytes)
            {
                for dep in dependency_declarations(text) {
                    if packages.contains(&dep.package) || targeted_dirs.contains(&dep.package) {
                        points.push(TouchPoint {
                            file: manifest_path.clone(),
                            kind: TouchKind::Dependency(dep.package),
                            line: dep.line,
                        });
                    }
                }
            }
            for (path, bytes) in &krate.files {
                if !is_rust_path(path) {
                    continue;
                }
                let Ok(text) = std::str::from_utf8(bytes) else { continue };
                let Ok(file) = syn::parse_file(text) else { continue };
                for declared in items_mentioning(&file, |ident| idents.contains(ident)) {
                    points.push(TouchPoint {
                        file: path.clone(),
                        kind: TouchKind::from(declared.head),
                        line: declared.line,
                    });
                }
            }
        }
        points
    }

    // Whether `dir` appears in root `Cargo.toml`, root `src/lib.rs`, or any
    // remaining stem manifest's `[dependencies]`.
    pub fn list_mentions(&self, dir: &str) -> Vec<String> {
        let package = dir;
        let ident = dir.replace('-', "_");
        let mut mentions = Vec::new();
        if let Some(manifest) = &self.root_cargo_toml {
            let needle = format!("crates/{dir}\"");
            for (idx, line) in manifest.lines().enumerate() {
                if uncommented(line).contains(&needle) {
                    mentions.push(format!("Cargo.toml:{}: `{}`", idx + 1, line.trim()));
                }
            }
        }
        if let Some(router) = &self.root_lib_rs {
            let needle = format!("{ident}::handlers");
            for (idx, line) in router.lines().enumerate() {
                if !line.trim_start().starts_with("//") && line.contains(&needle) {
                    mentions.push(format!("src/lib.rs:{}: `{}`", idx + 1, line.trim()));
                }
            }
        }
        for (other_dir, krate) in &self.crates {
            if other_dir == dir {
                continue;
            }
            let manifest_path = format!("crates/{other_dir}/Cargo.toml");
            if let Some(bytes) = krate.files.get(&manifest_path)
                && let Ok(text) = std::str::from_utf8(bytes)
            {
                for dep in dependency_declarations(text) {
                    if dep.package == package {
                        mentions.push(format!(
                            "{manifest_path}:{}: dependency `{}`",
                            dep.line, dep.package
                        ));
                    }
                }
            }
        }
        mentions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateLocality {
    pub dir: String,
    pub expected: Option<CrateAction>,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub byte_identical: bool,
    pub diff_stat: Vec<String>,
    pub touch_points: Vec<TouchPoint>,
    pub touched_items: Vec<String>,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendGrade {
    pub added: usize,
    pub changed: usize,
    pub retired: usize,
    pub stems: BTreeSet<String>,
    pub sections: BTreeSet<String>,
    pub parts_added: BTreeSet<String>,
    pub parts_removed: BTreeSet<String>,
    pub mismatches: Vec<String>,
}

impl AmendGrade {
    pub fn evaluate(amendment: &Value, expected: &ExpectedAmend) -> Self {
        let entries = amendment["changes"].as_array().map_or(&[][..], Vec::as_slice);
        let count_class = |wanted: &str| {
            entries.iter().filter(|change| change["class"].as_str() == Some(wanted)).count()
        };
        let added = count_class("added");
        let changed = count_class("changed");
        let retired = count_class("retired");

        let stems: BTreeSet<String> = entries
            .iter()
            .filter(|change| change["class"].as_str() != Some("unchanged"))
            .filter_map(|change| change["subject"].as_str())
            .map(|subject| subject.split_once('.').map_or(subject, |(stem, _)| stem).to_owned())
            .collect();

        let mut sections = BTreeSet::new();
        for key in ["added", "removed", "changed"] {
            sections.extend(strings(&amendment["sections"][key]));
        }

        let parts = amendment["parts"].as_array().map_or(&[][..], Vec::as_slice);
        let parts_by_class = |wanted: &str| -> BTreeSet<String> {
            parts
                .iter()
                .filter(|part| part["class"].as_str() == Some(wanted))
                .filter_map(|part| part["name"].as_str().map(str::to_owned))
                .collect()
        };
        let parts_added = parts_by_class("added");
        let parts_removed = parts_by_class("removed");

        let mut mismatches = Vec::new();
        if !expected.added.contains(added) {
            mismatches.push(format!("`added` is {added}, expected {}", expected.added));
        }
        if !expected.changed.contains(changed) {
            mismatches.push(format!("`changed` is {changed}, expected {}", expected.changed));
        }
        if !expected.retired.contains(retired) {
            mismatches.push(format!("`retired` is {retired}, expected {}", expected.retired));
        }
        if stems != expected.stems {
            mismatches.push(format!(
                "`stems` is [{}], expected [{}]",
                comma(&stems),
                comma(&expected.stems)
            ));
        }
        if !expected.sections.is_empty() && !expected.sections.is_subset(&sections) {
            mismatches.push(format!(
                "`sections` is [{}], expected to include [{}]",
                comma(&sections),
                comma(&expected.sections)
            ));
        }
        if parts_added != expected.parts_added {
            mismatches.push(format!(
                "`parts-added` is [{}], expected [{}]",
                comma(&parts_added),
                comma(&expected.parts_added)
            ));
        }
        if parts_removed != expected.parts_removed {
            mismatches.push(format!(
                "`parts-removed` is [{}], expected [{}]",
                comma(&parts_removed),
                comma(&expected.parts_removed)
            ));
        }

        Self {
            added,
            changed,
            retired,
            stems,
            sections,
            parts_added,
            parts_removed,
            mismatches,
        }
    }

    pub const fn passed(&self) -> bool {
        self.mismatches.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeGrade {
    pub whole_reason: Option<String>,
    pub minimum: BTreeSet<String>,
    pub widened: BTreeSet<String>,
    pub rebuilt: BTreeSet<String>,
    pub carried: BTreeSet<String>,
    pub retiring: BTreeSet<String>,
    pub mismatches: Vec<String>,
}

impl ScopeGrade {
    pub fn evaluate(
        envelope: &Value, plan: Option<&Value>, prior_plan: Option<&Value>,
        expected: &ExpectedScope,
    ) -> Self {
        let part_names = part_name_map(plan, prior_plan);
        let whole_reason = envelope["whole"]["reason"].as_str().map(str::to_owned);
        if let Some(reason) = whole_reason {
            return Self {
                whole_reason: Some(reason.clone()),
                minimum: BTreeSet::new(),
                widened: BTreeSet::new(),
                rebuilt: BTreeSet::new(),
                carried: BTreeSet::new(),
                retiring: BTreeSet::new(),
                mismatches: vec![format!("build ran as whole (`{reason}`), expected change build")],
            };
        }

        let change = &envelope["change"];
        let mut minimum = BTreeSet::new();
        let mut widened = BTreeSet::new();
        let mut rebuilt = BTreeSet::new();
        for entry in change["rebuilt"].as_array().into_iter().flatten() {
            let name = entry["name"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| entry["id"].as_str().and_then(|id| part_names.get(id).cloned()))
                .unwrap_or_default();
            let reasons = strings(&entry["reasons"]);
            if reasons.iter().all(|r| r == "adapter") {
                widened.insert(name.clone());
            } else {
                minimum.insert(name.clone());
            }
            rebuilt.insert(name);
        }

        let carried: BTreeSet<String> = strings(&change["carried"])
            .into_iter()
            .map(|id| part_names.get(&id).cloned().unwrap_or(id))
            .collect();

        let schedule = envelope["schedule"].as_array().map_or(&[][..], Vec::as_slice);
        let retiring: BTreeSet<String> = strings(&change["retiring"])
            .into_iter()
            .map(|slice_id| {
                schedule
                    .iter()
                    .find(|s| s["id"].as_str() == Some(slice_id.as_str()))
                    .and_then(|s| s["name"].as_str().map(str::to_owned))
                    .unwrap_or(slice_id)
            })
            .collect();

        let mut mismatches = Vec::new();
        if minimum != expected.minimum {
            mismatches.push(format!(
                "`minimum` is [{}], expected [{}]",
                comma(&minimum),
                comma(&expected.minimum)
            ));
        }
        if widened != expected.widened {
            mismatches.push(format!(
                "`widened` is [{}], expected [{}]",
                comma(&widened),
                comma(&expected.widened)
            ));
        }
        if rebuilt != expected.rebuilt {
            mismatches.push(format!(
                "`rebuilt` is [{}], expected [{}]",
                comma(&rebuilt),
                comma(&expected.rebuilt)
            ));
        }
        if retiring != expected.retiring {
            mismatches.push(format!(
                "`retiring` is [{}], expected [{}]",
                comma(&retiring),
                comma(&expected.retiring)
            ));
        }

        Self {
            whole_reason: None,
            minimum,
            widened,
            rebuilt,
            carried,
            retiring,
            mismatches,
        }
    }

    pub const fn passed(&self) -> bool {
        self.mismatches.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundTripDelta {
    pub against_step: usize,
    pub crate_dir: String,
    pub base: CrateSize,
    pub current: CrateSize,
}

impl RoundTripDelta {
    pub const fn loc_within_ten_percent(&self) -> bool {
        if self.base.loc == 0 {
            return self.current.loc == 0;
        }
        let diff = self.current.loc.abs_diff(self.base.loc);
        diff * 10 <= self.base.loc
    }

    pub fn loc_percent_delta(&self) -> isize {
        if self.base.loc == 0 {
            return 0;
        }
        let cur = isize::try_from(self.current.loc).unwrap_or(0);
        let base = isize::try_from(self.base.loc).unwrap_or(1);
        (cur - base) * 100 / base
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitShellCall {
    pub label: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionRef {
    pub label: String,
    pub commit: String,
}

// Inputs gathered at one evolution step to compare `before` and `after` git trees.
pub struct StepComparison<'a> {
    pub repo: &'a Path,
    pub step: usize,
    pub slug: &'a str,
    pub before: &'a TreeSnapshot,
    pub after: &'a TreeSnapshot,
    pub expected_crates: &'a BTreeMap<String, CrateAction>,
    pub amend: Option<AmendGrade>,
    pub scope: Option<ScopeGrade>,
    pub build_envelope: Option<&'a Value>,
    pub build_stderr: &'a str,
    pub round_trip_bases: &'a [(usize, &'a TreeSnapshot)],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepLocality {
    pub step: usize,
    pub slug: String,
    pub previous: Option<RevisionRef>,
    pub current: RevisionRef,
    pub sizes: BTreeMap<String, CrateSize>,
    pub crates: Vec<CrateLocality>,
    pub lingering_removals: BTreeMap<String, Vec<String>>,
    pub retiring_turns: usize,
    pub amend: Option<AmendGrade>,
    pub scope: Option<ScopeGrade>,
    pub round_trips: Vec<RoundTripDelta>,
    pub git_calls: Vec<GitShellCall>,
}

impl StepLocality {
    pub fn baseline(snapshot: &TreeSnapshot, build_stderr: &str) -> Self {
        let sizes =
            snapshot.crates.iter().map(|(dir, krate)| (dir.clone(), krate.size.clone())).collect();
        let crates = snapshot
            .crates
            .values()
            .map(|krate| CrateLocality {
                dir: krate.dir.clone(),
                expected: Some(CrateAction::Laid),
                before_hash: None,
                after_hash: Some(krate.tree_hash.clone()),
                byte_identical: false,
                diff_stat: Vec::new(),
                touch_points: Vec::new(),
                touched_items: Vec::new(),
                violations: Vec::new(),
            })
            .collect();
        Self {
            step: 0,
            slug: "00-baseline".to_owned(),
            previous: None,
            current: RevisionRef {
                label: snapshot.ref_name.clone(),
                commit: snapshot.commit.clone(),
            },
            sizes,
            crates,
            lingering_removals: BTreeMap::new(),
            retiring_turns: 0,
            amend: None,
            scope: None,
            round_trips: Vec::new(),
            git_calls: scan_git_shell_calls(build_stderr),
        }
    }

    pub fn compare(input: StepComparison<'_>) -> io::Result<Self> {
        let StepComparison {
            repo,
            step,
            slug,
            before,
            after,
            expected_crates,
            amend,
            scope,
            build_envelope,
            build_stderr,
            round_trip_bases,
        } = input;
        let sizes: BTreeMap<String, CrateSize> =
            after.crates.iter().map(|(dir, krate)| (dir.clone(), krate.size.clone())).collect();

        let targeted_dirs = targeted_crate_dirs(expected_crates, scope.as_ref(), before, after);
        let all_touch_points = before.touch_points(&targeted_dirs);

        let all_dirs: BTreeSet<String> = before
            .crates
            .keys()
            .chain(after.crates.keys())
            .chain(expected_crates.keys())
            .cloned()
            .collect();

        let mut crates = Vec::with_capacity(all_dirs.len());
        let mut lingering_removals = BTreeMap::new();
        for dir in all_dirs {
            let (locality, linger) = compare_crate(
                repo,
                &dir,
                expected_crates.get(&dir).copied(),
                before,
                after,
                &targeted_dirs,
                &all_touch_points,
            )?;
            if let Some(linger) = linger {
                lingering_removals.insert(dir, linger);
            }
            crates.push(locality);
        }

        let retiring_turns = count_retiring_turns(build_envelope, build_stderr);
        let round_trips = compute_round_trips(after, round_trip_bases);

        Ok(Self {
            step,
            slug: slug.to_owned(),
            previous: Some(RevisionRef {
                label: before.ref_name.clone(),
                commit: before.commit.clone(),
            }),
            current: RevisionRef {
                label: after.ref_name.clone(),
                commit: after.commit.clone(),
            },
            sizes,
            crates,
            lingering_removals,
            retiring_turns,
            amend,
            scope,
            round_trips,
            git_calls: scan_git_shell_calls(build_stderr),
        })
    }

    // Whether every crate expected `carried` is byte-identical to the prior step.
    pub fn carried_byte_identical(&self) -> bool {
        self.crates
            .iter()
            .filter(|c| c.expected == Some(CrateAction::Carried))
            .all(|c| c.byte_identical)
    }

    pub fn touch_violations(&self) -> Vec<&str> {
        self.crates
            .iter()
            .filter(|c| {
                matches!(c.expected, Some(CrateAction::Carried | CrateAction::Touch) | None)
            })
            .flat_map(|c| c.violations.iter().map(String::as_str))
            .collect()
    }

    pub fn removed_clean(&self) -> bool {
        self.lingering_removals.values().all(Vec::is_empty) && self.retiring_turns == 0
    }

    fn write_amend(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Some(amend) = &self.amend else {
            return Ok(());
        };
        let status = if amend.passed() { "pass" } else { "FAIL" };
        writeln!(
            f,
            "## Amend fidelity ({status})\n\n- added {}, changed {}, retired {}; stems [{}]; \
             sections [{}]; +parts [{}]; -parts [{}]",
            amend.added,
            amend.changed,
            amend.retired,
            comma(&amend.stems),
            comma(&amend.sections),
            comma(&amend.parts_added),
            comma(&amend.parts_removed),
        )?;
        for mismatch in &amend.mismatches {
            writeln!(f, "- mismatch: {mismatch}")?;
        }
        writeln!(f)
    }

    fn write_scope(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Some(scope) = &self.scope else {
            return Ok(());
        };
        let status = if scope.passed() { "pass" } else { "FAIL" };
        writeln!(f, "## Scope ({status})\n")?;
        if let Some(reason) = &scope.whole_reason {
            writeln!(f, "- whole build (`{reason}`)")?;
        } else {
            writeln!(
                f,
                "- minimum [{}], widened [{}], rebuilt [{}], carried [{}], retiring [{}]",
                comma(&scope.minimum),
                comma(&scope.widened),
                comma(&scope.rebuilt),
                comma(&scope.carried),
                comma(&scope.retiring),
            )?;
        }
        for mismatch in &scope.mismatches {
            writeln!(f, "- mismatch: {mismatch}")?;
        }
        writeln!(f)
    }

    fn write_crates_table(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "## Crates\n\n\
             | crate | expected | before | after | byte-identical | files | non-blank LOC | items | tests |\n\
             | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
        )?;
        for krate in &self.crates {
            let expected =
                krate.expected.map_or_else(|| "—".to_owned(), |action| action.to_string());
            let before = krate.before_hash.as_deref().map_or("—", short);
            let after = krate.after_hash.as_deref().map_or("—", short);
            let identical = if krate.before_hash.is_none() || krate.after_hash.is_none() {
                "—"
            } else if krate.byte_identical {
                "yes"
            } else {
                "no"
            };
            if let Some(size) = self.sizes.get(&krate.dir) {
                writeln!(
                    f,
                    "| `crates/{}` | {expected} | `{before}` | `{after}` | {identical} | {} | {} | {} | {} |",
                    krate.dir, size.files, size.loc, size.items, size.tests,
                )?;
            } else {
                writeln!(
                    f,
                    "| `crates/{}` | {expected} | `{before}` | `{after}` | {identical} | 0 | 0 | 0 | 0 |",
                    krate.dir,
                )?;
            }
        }
        for krate in &self.crates {
            if krate.diff_stat.is_empty()
                && krate.touch_points.is_empty()
                && krate.touched_items.is_empty()
                && krate.violations.is_empty()
            {
                continue;
            }
            writeln!(f, "\n### `crates/{}`", krate.dir)?;
            if !krate.touch_points.is_empty() {
                let listed: Vec<String> =
                    krate.touch_points.iter().map(ToString::to_string).collect();
                writeln!(f, "- pre-step touch points: {}", listed.join(", "))?;
            }
            if !krate.diff_stat.is_empty() {
                writeln!(f, "- git diff: {}", krate.diff_stat.join(", "))?;
            }
            for item in &krate.touched_items {
                writeln!(f, "- admitted touch-point edit: {item}")?;
            }
            for violation in &krate.violations {
                writeln!(f, "- violation: {violation}")?;
            }
        }
        Ok(())
    }

    fn write_tail_sections(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if !self.lingering_removals.is_empty() {
            writeln!(
                f,
                "\n## Removed crates ({})\n\n- retiring slice model turns: {}",
                if self.removed_clean() { "pass" } else { "FAIL" },
                self.retiring_turns
            )?;
            for (dir, linger) in &self.lingering_removals {
                if linger.is_empty() {
                    writeln!(f, "- `crates/{dir}`: removed cleanly from tree and root/stem lists")?;
                } else {
                    for hit in linger {
                        writeln!(f, "- `crates/{dir}` lingering: {hit}")?;
                    }
                }
            }
        }
        if !self.round_trips.is_empty() {
            writeln!(f, "\n## Round trip\n")?;
            for delta in &self.round_trips {
                writeln!(
                    f,
                    "- `crates/{}` vs step {:02}: LOC {} vs {} ({:+}%, within ±10%: {}), \
                     items {} vs {}, tests {} vs {}",
                    delta.crate_dir,
                    delta.against_step,
                    delta.current.loc,
                    delta.base.loc,
                    delta.loc_percent_delta(),
                    if delta.loc_within_ten_percent() { "yes" } else { "no" },
                    delta.current.items,
                    delta.base.items,
                    delta.current.tests,
                    delta.base.tests,
                )?;
            }
        }
        writeln!(f, "\n## Worker `git` shell scan\n")?;
        if self.git_calls.is_empty() {
            writeln!(f, "- none: 0 `git` shell invocations in build/verify trace.")
        } else {
            writeln!(f, "- {} `git` shell invocation(s):", self.git_calls.len())?;
            for call in &self.git_calls {
                writeln!(f, "  - `{}`: `{}`", call.label, call.command)?;
            }
            Ok(())
        }
    }
}

impl Display for StepLocality {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "# Locality — `{}`\n", self.slug)?;
        match &self.previous {
            Some(prev) => writeln!(
                f,
                "Diff: `{}` (`{}`) → `{}` (`{}`)\n",
                prev.label,
                short(&prev.commit),
                self.current.label,
                short(&self.current.commit),
            )?,
            None => writeln!(
                f,
                "Baseline: `{}` (`{}`)\n",
                self.current.label,
                short(&self.current.commit),
            )?,
        }
        self.write_amend(f)?;
        self.write_scope(f)?;
        self.write_crates_table(f)?;
        self.write_tail_sections(f)
    }
}

fn targeted_crate_dirs(
    expected_crates: &BTreeMap<String, CrateAction>, scope: Option<&ScopeGrade>,
    before: &TreeSnapshot, after: &TreeSnapshot,
) -> BTreeSet<String> {
    let mut targeted: BTreeSet<String> = expected_crates
        .iter()
        .filter(|(_, action)| matches!(action, CrateAction::Rewrite | CrateAction::Removed))
        .map(|(dir, _)| dir.clone())
        .collect();
    if let Some(scope) = scope {
        targeted.extend(scope.rebuilt.iter().cloned());
        targeted.extend(scope.retiring.iter().cloned());
    }
    for dir in before.crates.keys() {
        if !after.crates.contains_key(dir) {
            targeted.insert(dir.clone());
        }
    }
    targeted
}

fn compare_crate(
    repo: &Path, dir: &str, expected: Option<CrateAction>, before: &TreeSnapshot,
    after: &TreeSnapshot, targeted_dirs: &BTreeSet<String>, all_touch_points: &[TouchPoint],
) -> io::Result<(CrateLocality, Option<Vec<String>>)> {
    let prev = before.crates.get(dir);
    let curr = after.crates.get(dir);
    let before_hash = prev.map(|c| c.tree_hash.clone());
    let after_hash = curr.map(|c| c.tree_hash.clone());
    let byte_identical = before_hash.is_some() && before_hash == after_hash;
    let diff_stat = if byte_identical || (prev.is_none() && curr.is_none()) {
        Vec::new()
    } else {
        git_lines(
            repo,
            &[
                "diff",
                "--name-status",
                &before.commit,
                &after.commit,
                "--",
                &format!("crates/{dir}"),
            ],
        )?
    };

    let crate_prefix = format!("crates/{dir}/");
    let touch_points: Vec<TouchPoint> =
        all_touch_points.iter().filter(|tp| tp.file.starts_with(&crate_prefix)).cloned().collect();

    let check_outside = !targeted_dirs.contains(dir)
        && !matches!(
            expected,
            Some(CrateAction::Laid | CrateAction::Rewrite | CrateAction::Removed)
        );
    let (touched_items, mut violations) = if check_outside && !byte_identical {
        if let (Some(prev), Some(curr)) = (prev, curr) {
            diff_non_rebuilt_crate(prev, curr, &touch_points)
        } else {
            (Vec::new(), vec![format!("`crates/{dir}` appeared or vanished unexpectedly")])
        }
    } else {
        (Vec::new(), Vec::new())
    };

    let mut removed_linger = None;
    match expected {
        Some(CrateAction::Carried) if !byte_identical => {
            if violations.is_empty() && touch_points.is_empty() {
                violations.push(format!(
                    "`crates/{dir}` expected `carried` byte-identical, but tree hash moved"
                ));
            }
        }
        Some(CrateAction::Removed) => {
            let mut linger = Vec::new();
            if curr.is_some() {
                linger.push(format!("`crates/{dir}/` still exists in tree"));
            }
            linger.extend(after.list_mentions(dir));
            if !linger.is_empty() {
                violations.extend(linger.iter().cloned());
            }
            removed_linger = Some(linger);
        }
        Some(CrateAction::Laid) if curr.is_none() => {
            violations.push(format!("`crates/{dir}/` expected `laid`, but absent"));
        }
        _ => {}
    }

    Ok((
        CrateLocality {
            dir: dir.to_owned(),
            expected,
            before_hash,
            after_hash,
            byte_identical,
            diff_stat,
            touch_points,
            touched_items,
            violations,
        },
        removed_linger,
    ))
}

fn compute_round_trips(
    after: &TreeSnapshot, round_trip_bases: &[(usize, &TreeSnapshot)],
) -> Vec<RoundTripDelta> {
    let mut deltas = Vec::new();
    for &(base_step, base_snap) in round_trip_bases {
        if let (Some(base_orders), Some(curr_orders)) =
            (base_snap.crates.get("orders"), after.crates.get("orders"))
        {
            deltas.push(RoundTripDelta {
                against_step: base_step,
                crate_dir: "orders".to_owned(),
                base: base_orders.size.clone(),
                current: curr_orders.size.clone(),
            });
        }
    }
    deltas
}

// Scans backend tool-call trace lines for shell invocations running `git`.
pub fn scan_git_shell_calls(stderr: &str) -> Vec<GitShellCall> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    for line in stderr.lines() {
        let label = span_label(line).unwrap_or("unknown");
        if (line.contains("omnia_cursor::tools:") || line.contains("omnia_genai::tools:"))
            && line.contains("tool call args")
            && line.contains("tool=shell")
            && let Some((_, args_text)) = line.split_once("args=")
            && let Ok(args) = serde_json::from_str::<Value>(args_text.trim())
            && let Some(command) = args.get("command").and_then(Value::as_str)
        {
            if invokes_git(command) {
                let one_line = flatten_ws(command);
                if seen.insert((label.to_owned(), one_line.clone())) {
                    found.push(GitShellCall {
                        label: label.to_owned(),
                        command: one_line,
                    });
                }
            }
            continue;
        }
        if let Some((_, tail)) = line.split_once(" tool call tool=shell")
            && let Some(subject) = quoted_or_bare_field(tail, "subject=")
            && invokes_git(&subject)
        {
            let one_line = flatten_ws(&subject);
            if seen.insert((label.to_owned(), one_line.clone())) {
                found.push(GitShellCall {
                    label: label.to_owned(),
                    command: one_line,
                });
            }
        }
    }
    found
}

fn invokes_git(command: &str) -> bool {
    // Splits shell command segments (`;`, `&&`, `||`, `|`, `(`, `` ` ``, `\n`)
    // and checks whether any segment's leading command (past `env` / `FOO=bar`)
    // is `git`.
    for segment in command.split([';', '|', '&', '\n', '(', '`']) {
        for word in segment.split_whitespace() {
            if word == "env" || (word.contains('=') && !word.starts_with('-')) {
                continue;
            }
            if word == "git" || word.ends_with("/git") {
                return true;
            }
            break;
        }
    }
    false
}

fn flatten_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn span_label(line: &str) -> Option<&str> {
    let (_, rest) = line.split_once("label=\"")?;
    let (label, _) = rest.split_once('"')?;
    Some(label)
}

fn quoted_or_bare_field(text: &str, key: &str) -> Option<String> {
    let (_, rest) = text.split_once(key)?;
    let rest = rest.trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        let mut out = String::new();
        let mut escaped = false;
        for ch in quoted.chars() {
            if escaped {
                out.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Some(out);
            } else {
                out.push(ch);
            }
        }
        Some(out)
    } else {
        rest.split_whitespace().next().map(str::to_owned)
    }
}

fn count_retiring_turns(envelope: Option<&Value>, stderr: &str) -> usize {
    let Some(envelope) = envelope else {
        return 0;
    };
    let retiring: BTreeSet<String> = strings(&envelope["change"]["retiring"]).into_iter().collect();
    if retiring.is_empty() {
        return 0;
    }
    crate::run::completions(stderr)
        .into_iter()
        .filter(|c| c.label.strip_prefix("build-").is_some_and(|slice| retiring.contains(slice)))
        .count()
}

fn part_name_map(plan: Option<&Value>, prior_plan: Option<&Value>) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    for doc in [prior_plan, plan].into_iter().flatten() {
        for part in doc["parts"].as_array().into_iter().flatten() {
            if let (Some(id), Some(name)) = (part["id"].as_str(), part["name"].as_str()) {
                names.insert(id.to_owned(), name.to_owned());
            }
        }
    }
    names
}

fn diff_non_rebuilt_crate(
    before: &CrateSnapshot, after: &CrateSnapshot, touch_points: &[TouchPoint],
) -> (Vec<String>, Vec<String>) {
    let paths: BTreeSet<&String> = before.files.keys().chain(after.files.keys()).collect();
    let mut admitted = Vec::new();
    let mut violations = Vec::new();
    for path in paths {
        let prev = before.files.get(path);
        let curr = after.files.get(path);
        match (prev, curr) {
            (Some(prev), Some(curr)) if prev == curr => {}
            (None, Some(_)) => {
                violations.push(format!("{path}: added outside rebuilt crates"));
            }
            (Some(_), None) => {
                violations.push(format!("{path}: removed outside rebuilt crates"));
            }
            (None, None) => {}
            (Some(prev), Some(curr)) => {
                let file_points: Vec<TouchPoint> =
                    touch_points.iter().filter(|tp| tp.file == *path).cloned().collect();
                let before_text = String::from_utf8_lossy(prev);
                let after_text = String::from_utf8_lossy(curr);
                if is_rust_path(path) {
                    let (ok, bad) = diff_rust_items(path, &before_text, &after_text, &file_points);
                    admitted.extend(ok);
                    violations.extend(bad);
                } else if path.ends_with("/Cargo.toml") {
                    let (ok, bad) =
                        diff_manifest_deps(path, &before_text, &after_text, &file_points);
                    admitted.extend(ok);
                    violations.extend(bad);
                } else {
                    violations.push(format!("{path}: changed, and it is no touch point"));
                }
            }
        }
    }
    (admitted, violations)
}

fn diff_rust_items(
    path: &str, before: &str, after: &str, points: &[TouchPoint],
) -> (Vec<String>, Vec<String>) {
    let (Ok(before_file), Ok(after_file)) = (syn::parse_file(before), syn::parse_file(after))
    else {
        return (Vec::new(), vec![format!("{path}: changed and failed to parse as Rust")]);
    };
    let mut prior: Vec<&Item> = before_file.items.iter().collect();
    let mut after_only = Vec::new();
    for item in &after_file.items {
        if let Some(at) = prior.iter().position(|old| *old == item) {
            prior.remove(at);
        } else {
            after_only.push(declared_item(item));
        }
    }
    let before_only: Vec<DeclaredItem> = prior.into_iter().map(declared_item).collect();

    let mut admitted = Vec::new();
    let mut violations = Vec::new();
    if before_file.attrs != after_file.attrs {
        violations.push(format!("{path}:1: inner attributes changed"));
    }
    for item in &after_only {
        let what =
            if before_only.iter().any(|old| old.head == item.head) { "changed" } else { "added" };
        if TouchPoint::admits_item(points, &item.head) {
            admitted.push(format!("{path}:{}: `{}` {what}", item.line, item.head));
        } else {
            violations.push(format!(
                "{path}:{}: `{}` {what}, and it is no touch point",
                item.line, item.head
            ));
        }
    }
    for item in &before_only {
        if after_only.iter().any(|new| new.head == item.head) {
            continue;
        }
        if TouchPoint::admits_item(points, &item.head) {
            admitted.push(format!("{path}:{}: `{}` removed", item.line, item.head));
        } else {
            violations.push(format!(
                "{path}:{}: `{}` removed, and it is no touch point",
                item.line, item.head
            ));
        }
    }
    (admitted, violations)
}

fn diff_manifest_deps(
    path: &str, before: &str, after: &str, points: &[TouchPoint],
) -> (Vec<String>, Vec<String>) {
    if lines_outside_dependencies(before) != lines_outside_dependencies(after) {
        return (Vec::new(), vec![format!("{path}: lines outside `[dependencies]` changed")]);
    }
    let before_deps = dependency_entries(before);
    let after_deps = dependency_entries(after);
    let mut admitted = Vec::new();
    let mut violations = Vec::new();
    for dep in &after_deps {
        if !before_deps.iter().any(|old| old.entry == dep.entry) {
            if TouchPoint::admits_dependency(points, &dep.package) {
                admitted.push(format!("{path}:{}: `{}` added", dep.line, dep.entry));
            } else {
                violations.push(format!(
                    "{path}:{}: `{}` added, and it is no touch point",
                    dep.line, dep.entry
                ));
            }
        }
    }
    for dep in &before_deps {
        if !after_deps.iter().any(|new| new.entry == dep.entry) {
            if TouchPoint::admits_dependency(points, &dep.package) {
                admitted.push(format!("{path}:{}: `{}` removed", dep.line, dep.entry));
            } else {
                violations.push(format!(
                    "{path}:{}: `{}` removed, and it is no touch point",
                    dep.line, dep.entry
                ));
            }
        }
    }
    (admitted, violations)
}

fn is_rust_path(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

fn measure_crate(files: &BTreeMap<String, Vec<u8>>) -> CrateSize {
    let mut rust_files = 0;
    let mut loc = 0;
    let mut items = 0;
    let mut tests = 0;
    for (path, bytes) in files {
        if !is_rust_path(path) {
            continue;
        }
        rust_files += 1;
        let text = String::from_utf8_lossy(bytes);
        loc += text.lines().filter(|line| !line.trim().is_empty()).count();
        if let Ok(parsed) = syn::parse_file(&text) {
            items += parsed.items.len();
            tests += count_test_fns(&parsed.items);
        }
    }
    CrateSize {
        files: rust_files,
        loc,
        items,
        tests,
    }
}

fn count_test_fns(items: &[Item]) -> usize {
    let mut count = 0;
    for item in items {
        match item {
            Item::Fn(func) => {
                if func.attrs.iter().any(is_test_attr) {
                    count += 1;
                }
            }
            Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    count += count_test_fns(inner);
                }
            }
            _ => {}
        }
    }
    count
}

fn is_test_attr(attr: &syn::Attribute) -> bool {
    attr.path().segments.last().is_some_and(|seg| seg.ident == "test")
}

fn read_crate_files(repo: &Path, commit: &str, dir: &str) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let prefix = format!("crates/{dir}");
    let paths = git_lines(repo, &["ls-tree", "-r", "--name-only", commit, &prefix])?;
    let mut files = BTreeMap::new();
    for path in paths {
        if let Some(bytes) = git_file(repo, commit, &path)? {
            files.insert(path, bytes);
        }
    }
    Ok(files)
}

fn git_lines(repo: &Path, args: &[&str]) -> io::Result<Vec<String>> {
    let out = git(repo, args)?;
    if out.is_empty() {
        return Ok(Vec::new());
    }
    Ok(out.lines().filter(|line| !line.is_empty()).map(str::to_owned).collect())
}

fn git_utf8(repo: &Path, commit: &str, path: &str) -> io::Result<Option<String>> {
    Ok(git_file(repo, commit, path)?.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
}

fn manifest_package_name(text: &str) -> Option<String> {
    let table: toml::Table = text.parse().ok()?;
    table.get("package")?.get("name")?.as_str().map(str::to_owned)
}

fn comma(items: &BTreeSet<String>) -> String {
    items.iter().map(String::as_str).collect::<Vec<_>>().join(", ")
}

// --- TOML `[dependencies]` helpers matching `adapters/omnia/src/layout/manifest.rs` ---

struct DepDecl {
    line: usize,
    package: String,
}

struct DepEntry {
    line: usize,
    package: String,
    entry: String,
}

fn dependency_declarations(text: &str) -> Vec<DepDecl> {
    let mut table = String::new();
    let mut named: Vec<(usize, String)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = uncommented(raw).trim();
        if line.starts_with('[') {
            line.trim_start_matches('[').trim_end_matches(']').trim().clone_into(&mut table);
            if let Some(dep) = table.strip_prefix("dependencies.") {
                let pkg = unquoted(dep).to_owned();
                if !pkg.is_empty() {
                    named.push((index + 1, pkg));
                }
            }
        } else if table == "dependencies"
            && let Some((name, _)) = line.split_once('=')
        {
            let head = name.trim().split('.').next().unwrap_or_default();
            let pkg = unquoted(head).to_owned();
            if !pkg.is_empty() {
                named.push((index + 1, pkg));
            }
        }
    }
    named.sort_by_key(|(line, _)| *line);
    let mut found: Vec<DepDecl> = Vec::new();
    for (line, package) in named {
        if !found.iter().any(|seen| seen.package == package) {
            found.push(DepDecl { line, package });
        }
    }
    found
}

fn dependency_entries(text: &str) -> Vec<DepEntry> {
    let mut entries = Vec::new();
    let mut in_dependencies = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = uncommented(line).trim();
        if trimmed.starts_with('[') {
            let table = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
            in_dependencies = table == "dependencies";
        } else if in_dependencies && !trimmed.is_empty() {
            let key = trimmed.split_once('=').map_or(trimmed, |(key, _)| key);
            let head = key.split('.').next().unwrap_or(key);
            entries.push(DepEntry {
                line: index + 1,
                package: unquoted(head).to_owned(),
                entry: trimmed.to_owned(),
            });
        }
    }
    entries
}

fn lines_outside_dependencies(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_dependencies = false;
    for line in text.lines() {
        let trimmed = uncommented(line).trim();
        if trimmed.starts_with('[') {
            let table = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
            in_dependencies = table == "dependencies";
            out.push(line);
        } else if !in_dependencies {
            out.push(line);
        }
    }
    out
}

fn uncommented(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (at, ch) in line.char_indices() {
        match quote {
            Some(_) if escaped => escaped = false,
            Some('"') if ch == '\\' => escaped = true,
            Some(open) if ch == open => quote = None,
            None if matches!(ch, '"' | '\'') => quote = Some(ch),
            None if ch == '#' => return &line[..at],
            _ => {}
        }
    }
    line
}

fn unquoted(key: &str) -> &str {
    key.trim().trim_matches(|ch| ch == '"' || ch == '\'')
}

// --- `syn` item and crate-mention readers matching `adapters/omnia/src/syntax.rs` ---

#[derive(Clone, Debug, PartialEq, Eq)]
enum ItemHead {
    Use,
    Named(String),
}

impl Display for ItemHead {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Use => f.write_str("use"),
            Self::Named(name) => f.write_str(name),
        }
    }
}

impl From<ItemHead> for TouchKind {
    fn from(head: ItemHead) -> Self {
        match head {
            ItemHead::Use => Self::Use,
            ItemHead::Named(name) => Self::Item(name),
        }
    }
}

struct DeclaredItem {
    line: usize,
    head: ItemHead,
}

fn span_line(span: Span) -> usize {
    span.start().line
}

fn declared_item(item: &Item) -> DeclaredItem {
    match item {
        Item::Use(item) => DeclaredItem {
            line: span_line(item.use_token.span),
            head: ItemHead::Use,
        },
        Item::ExternCrate(item) => DeclaredItem {
            line: span_line(item.extern_token.span),
            head: ItemHead::Named(format!("extern crate {}", item.ident)),
        },
        Item::ForeignMod(item) => DeclaredItem {
            line: span_line(item.abi.extern_token.span),
            head: ItemHead::Named("extern".to_owned()),
        },
        _ => {
            let (line, name) = item_head(item).unwrap_or_else(|| (1, "item".to_owned()));
            DeclaredItem {
                line,
                head: ItemHead::Named(name),
            }
        }
    }
}

fn item_head(item: &Item) -> Option<(usize, String)> {
    let (span, head) = match item {
        Item::Fn(item) => (item.sig.fn_token.span, format!("fn {}", item.sig.ident)),
        Item::Struct(item) => (item.struct_token.span, format!("struct {}", item.ident)),
        Item::Enum(item) => (item.enum_token.span, format!("enum {}", item.ident)),
        Item::Union(item) => (item.union_token.span, format!("union {}", item.ident)),
        Item::Const(item) => (item.const_token.span, format!("const {}", item.ident)),
        Item::Static(item) => (item.static_token.span, format!("static {}", item.ident)),
        Item::Type(item) => (item.type_token.span, format!("type {}", item.ident)),
        Item::Trait(item) => (item.trait_token.span, format!("trait {}", item.ident)),
        Item::TraitAlias(item) => (item.trait_token.span, format!("trait {}", item.ident)),
        Item::Mod(item) => (item.mod_token.span, format!("mod {}", item.ident)),
        Item::Impl(item) => {
            let name = match &*item.self_ty {
                syn::Type::Path(path) => {
                    path.path.segments.last().map(|segment| segment.ident.to_string())
                }
                _ => None,
            };
            let head = name.map_or_else(|| "impl".to_owned(), |name| format!("impl {name}"));
            (item.impl_token.span, head)
        }
        Item::Macro(item) => {
            let segment = item.mac.path.segments.last()?;
            (segment.ident.span(), format!("{}!", segment.ident))
        }
        _ => return None,
    };
    Some((span_line(span), head))
}

fn items_mentioning(file: &syn::File, matches_crate: impl Fn(&str) -> bool) -> Vec<DeclaredItem> {
    let mut imported = BTreeSet::new();
    for item in &file.items {
        if let Item::Use(item) = item {
            collect_imported(&item.tree, None, false, &matches_crate, &mut imported);
        }
    }

    let mut found = Vec::new();
    for item in &file.items {
        let mentions_target = if let Item::Use(item) = item {
            use_mentions(&item.tree, &matches_crate)
        } else {
            let mentions = ItemMentions::of(item);
            mentions.paths.iter().any(|path| path.first().is_some_and(|head| matches_crate(head)))
                || imported.iter().any(|symbol| mentions.idents.contains(symbol))
        };
        if mentions_target {
            found.push(declared_item(item));
        }
    }
    found
}

#[derive(Default)]
struct ItemMentions {
    idents: BTreeSet<String>,
    paths: Vec<Vec<String>>,
}

impl ItemMentions {
    fn of(item: &Item) -> Self {
        let mut mentions = Self::default();
        mentions.visit_item(item);
        mentions
    }

    fn push_path(&mut self, segments: Vec<String>) {
        if segments.len() > 1 {
            self.paths.push(segments);
        }
    }

    fn tokens(&mut self, stream: TokenStream) {
        let mut path: Vec<String> = Vec::new();
        let mut colons = 0;
        for token in stream {
            match token {
                TokenTree::Ident(ident) => {
                    self.idents.insert(ident.to_string());
                    if colons != 2 {
                        self.push_path(std::mem::take(&mut path));
                    }
                    path.push(ident.to_string());
                    colons = 0;
                }
                TokenTree::Punct(punct) if punct.as_char() == ':' => colons += 1,
                TokenTree::Group(group) => {
                    self.push_path(std::mem::take(&mut path));
                    colons = 0;
                    self.tokens(group.stream());
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {
                    self.push_path(std::mem::take(&mut path));
                    colons = 0;
                }
            }
        }
        self.push_path(path);
    }
}

impl<'ast> Visit<'ast> for ItemMentions {
    fn visit_ident(&mut self, node: &'ast Ident) {
        self.idents.insert(node.to_string());
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.push_path(node.segments.iter().map(|s| s.ident.to_string()).collect());
        visit::visit_path(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let mut paths = Vec::new();
        use_paths(&node.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            self.push_path(path);
        }
        visit::visit_item_use(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        self.tokens(node.tokens.clone());
        visit::visit_macro(self, node);
    }

    fn visit_meta_list(&mut self, node: &'ast MetaList) {
        self.tokens(node.tokens.clone());
        visit::visit_meta_list(self, node);
    }
}

fn use_paths(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    let leaf = |prefix: &[String], last: &str| {
        prefix.iter().cloned().chain([last.to_owned()]).collect::<Vec<_>>()
    };
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            use_paths(&path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(name) => out.push(leaf(prefix, &name.ident.to_string())),
        UseTree::Rename(rename) => out.push(leaf(prefix, &rename.ident.to_string())),
        UseTree::Glob(_) => out.push(leaf(prefix, "*")),
        UseTree::Group(group) => {
            for item in &group.items {
                use_paths(item, prefix, out);
            }
        }
    }
}

fn collect_imported(
    tree: &UseTree, parent: Option<&Ident>, under_target: bool,
    matches_crate: &impl Fn(&str) -> bool, symbols: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            let targeted = under_target || matches_crate(&path.ident.to_string());
            if targeted || parent.is_none() {
                collect_imported(&path.tree, Some(&path.ident), targeted, matches_crate, symbols);
            }
        }
        UseTree::Name(name) if under_target => {
            if name.ident == "self" {
                if let Some(module) = parent {
                    let module = module.to_string();
                    if !matches_crate(&module) {
                        symbols.insert(module);
                    }
                }
            } else {
                symbols.insert(name.ident.to_string());
            }
        }
        UseTree::Rename(rename)
            if (under_target || (parent.is_none() && matches_crate(&rename.ident.to_string())))
                && rename.rename != "_" =>
        {
            symbols.insert(rename.rename.to_string());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_imported(item, parent, under_target, matches_crate, symbols);
            }
        }
        _ => {}
    }
}

fn use_mentions(tree: &UseTree, matches_crate: &impl Fn(&str) -> bool) -> bool {
    match tree {
        UseTree::Path(path) => matches_crate(&path.ident.to_string()),
        UseTree::Name(name) => matches_crate(&name.ident.to_string()),
        UseTree::Rename(rename) => matches_crate(&rename.ident.to_string()),
        UseTree::Group(group) => group.items.iter().any(|item| use_mentions(item, matches_crate)),
        UseTree::Glob(_) => false,
    }
}

// Writes `step-K.locality.md` and returns its rendered Markdown.
pub fn write_locality(project: &Path, tag: &str, locality: &StepLocality) -> io::Result<String> {
    let text = locality.to_string();
    fs::write(project.join(format!("{tag}.locality.md")), &text)?;
    Ok(text)
}

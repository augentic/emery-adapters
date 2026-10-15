//! The reference comparison: the hand-written omnia guest a case names,
//! which a build through `omnia` is measured against, and whose recorded
//! fixtures the built guest is driven through as a behavioural oracle.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::{HostCheck, files_under, host_check};

// The reference guest a case names, located under the repository root.
pub struct Reference {
    dir: PathBuf,
    name: String,
}

impl Reference {
    // The reference `named` relative to `root`, where its tree exists; one
    // the root lacks is reported and left out, since a reference is
    // vendored beside its fixture and may be absent.
    pub fn locate(root: &Path, named: Option<&str>) -> Option<Self> {
        let name = named?;
        let dir = root.join(name);
        if !dir.is_dir() {
            eprintln!("eval: reference `{name}` is not a directory; nothing compared");
            return None;
        }
        Some(Self {
            dir,
            name: name.to_owned(),
        })
    }

    // The built tree at `export` measured against the reference, and the
    // reference's fixtures driven through the guest it holds.
    pub fn compare(&self, export: &Path) -> io::Result<Comparison> {
        let built = Shape::read(export)?;
        let reference = Shape::read(&self.dir)?;
        let measures = vec![
            Measure::names("crates", &built.crates, &reference.crates),
            Measure::count("Rust files", built.files, reference.files),
            Measure::count("Rust lines (non-blank)", built.lines, reference.lines),
            Measure::names("dependencies", &built.dependencies, &reference.dependencies),
            Measure::names("config keys", &built.config, &reference.config),
            Measure::names("topics", &built.topics, &reference.topics),
        ];
        let (oracle, log) = match self.stage(export, &built)? {
            Ok(fixtures) => {
                let check = host_check(
                    export,
                    "cargo test",
                    &["--test", HARNESS, "--", "--nocapture", "--test-threads=1"],
                );
                (Oracle::read(&check, fixtures), check.output)
            }
            Err(why) => (Oracle::not_run(why), String::new()),
        };
        Ok(Comparison {
            name: self.name.clone(),
            measures,
            oracle,
            log,
        })
    }

    // Every recorded fixture of the reference, as `<group>/<name>` and its
    // path, the groups being `static` and `replay` under any crate's `data/`.
    fn fixtures(&self) -> io::Result<Vec<(String, PathBuf)>> {
        let mut found = Vec::new();
        let crates = self.dir.join("crates");
        for entry in fs::read_dir(&crates).into_iter().flatten() {
            let data = entry?.path().join("data");
            for group in GROUPS {
                let Ok(entries) = fs::read_dir(data.join(group)) else {
                    continue;
                };
                for entry in entries {
                    let path = entry?.path();
                    if path.extension().is_some_and(|extension| extension == "json")
                        && let Some(stem) = path.file_stem()
                    {
                        found.push((format!("{group}/{}", stem.to_string_lossy()), path));
                    }
                }
            }
        }
        found.sort();
        Ok(found)
    }

    // The harness laid into the export: the fixtures copied under
    // `oracle/`, the settings the harness reads, the dev-dependencies it
    // needs, and the test itself; `Err` names why it cannot run there.
    fn stage(&self, export: &Path, built: &Shape) -> io::Result<Result<usize, String>> {
        let fixtures = self.fixtures()?;
        if fixtures.is_empty() {
            return Ok(Err(format!(
                "`{}` holds no fixtures under `crates/*/data/{{static,replay}}/`",
                self.name
            )));
        }
        let manifest_path = export.join("Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path)?;
        let table: toml::Table = manifest
            .parse()
            .map_err(|error| io::Error::other(format!("the built tree's Cargo.toml: {error}")))?;
        let Some(package) = table
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
        else {
            return Ok(Err("the built tree's root `Cargo.toml` declares no package".to_owned()));
        };
        let additions = match dev_dependency_additions(&table) {
            Ok(additions) => additions,
            Err(why) => return Ok(Err(why)),
        };

        // the fixtures and the settings
        let oracle = export.join("oracle");
        for group in GROUPS {
            fs::create_dir_all(oracle.join(group))?;
        }
        for (name, path) in &fixtures {
            fs::copy(path, oracle.join(format!("{name}.json")))?;
        }
        let config = config_seed(&built.config);
        let settings = serde_json::json!({
            "inbound": inbound_candidates(&built.topics, &config),
            "config": config,
        });
        fs::write(oracle.join("oracle.json"), serde_json::to_string_pretty(&settings)?)?;

        // the harness, over the dev-dependencies it reads
        if !additions.is_empty() {
            fs::write(&manifest_path, format!("{manifest}\n{additions}"))?;
        }
        fs::create_dir_all(export.join("tests"))?;
        fs::write(
            export.join(harness_path()),
            HARNESS_SOURCE.replace("__CRATE__", &package.replace('-', "_")),
        )?;
        Ok(Ok(fixtures.len()))
    }
}

const GROUPS: [&str; 2] = ["static", "replay"];

const HARNESS: &str = "reference_oracle";

fn harness_path() -> String {
    format!("tests/{HARNESS}.rs")
}

// The TOML the harness's dev-dependencies are appended to the built tree's
// root manifest as, one table per crate the root package depends on under
// no table: the workspace's pin where it carries one, a version otherwise.
// `omnia-test` and `tokio` are the layout's own and are never added, since
// their versions and features are the tree's to pin.
fn dev_dependency_additions(manifest: &toml::Table) -> Result<String, String> {
    const NEEDED: [(&str, &str); 6] = [
        ("bytes", "1"),
        ("chrono", "0.4"),
        ("chrono-tz", "0.10"),
        ("http", "1"),
        ("serde_json", "1"),
        ("tracing", "0.1"),
    ];
    const NATIVE: &str = "target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies";

    let present = package_dependency_names(manifest);
    for required in ["omnia-test", "tokio"] {
        if !present.contains(required) {
            return Err(format!(
                "the built tree's root package has no `{required}` dev-dependency"
            ));
        }
    }
    let workspace: BTreeSet<&str> = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let mut additions = String::new();
    for (name, version) in NEEDED {
        if present.contains(name) {
            continue;
        }
        let entry = if workspace.contains(name) {
            "workspace = true".to_owned()
        } else {
            format!("version = \"{version}\"")
        };
        let _ = write!(additions, "[{NATIVE}.{name}]\n{entry}\n\n");
    }
    Ok(additions)
}

// Every crate the manifest's package depends on, as a dependency or a
// dev-dependency, under any target; the workspace's table declares none.
fn package_dependency_names(manifest: &toml::Table) -> BTreeSet<String> {
    const TABLES: [&str; 2] = ["dependencies", "dev-dependencies"];
    let mut names: BTreeSet<String> = BTreeSet::new();
    for table in TABLES {
        names.extend(table_keys(manifest.get(table)));
        for target in manifest.get("target").and_then(toml::Value::as_table).into_iter().flatten()
        {
            names.extend(table_keys(target.1.get(table)));
        }
    }
    names
}

fn table_keys(value: Option<&toml::Value>) -> BTreeSet<String> {
    value
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default()
}

// What a guest's tree spells, measured alike on both sides: the crates
// under `crates/`, its Rust files and their non-blank lines, the external
// crates its manifests depend on, and the configuration keys and topic
// suffixes of its `common` crate's catalogs.
struct Shape {
    crates: BTreeSet<String>,
    files: usize,
    lines: usize,
    dependencies: BTreeSet<String>,
    config: BTreeSet<String>,
    topics: BTreeSet<String>,
}

impl Shape {
    // The tree read less what an earlier comparison staged into it, so an
    // export kept and compared again measures the same.
    fn read(root: &Path) -> io::Result<Self> {
        let mut paths = Vec::new();
        files_under(root, root, "target", &mut paths)?;
        paths.retain(|path| !path.starts_with("oracle/") && *path != harness_path());
        paths.sort();

        // the crates and the Rust sources
        let crates = paths
            .iter()
            .filter_map(|path| path.strip_prefix("crates/"))
            .filter_map(|rest| rest.split_once('/'))
            .map(|(name, _)| name.to_owned())
            .collect();
        let rust: Vec<&String> = paths
            .iter()
            .filter(|path| {
                Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
            })
            .collect();
        let mut lines = 0;
        for path in &rust {
            lines += fs::read_to_string(root.join(path))?
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
        }

        // the dependencies, less the tree's own packages
        let mut packages = BTreeSet::new();
        let mut named = BTreeSet::new();
        for path in paths.iter().filter(|path| path.rsplit('/').next() == Some("Cargo.toml")) {
            let table: toml::Table = fs::read_to_string(root.join(path))?
                .parse()
                .map_err(|error| io::Error::other(format!("{path}: {error}")))?;
            if let Some(name) = table
                .get("package")
                .and_then(|package| package.get("name"))
                .and_then(toml::Value::as_str)
            {
                packages.insert(name.to_owned());
            }
            named.extend(dependency_names(&table));
        }
        let dependencies = named.difference(&packages).cloned().collect();

        // the catalogs
        let common = root.join("crates/common/src");
        let config = str_consts(&read_or_empty(&common.join("config.rs")));
        let mut topics = str_consts(&read_or_empty(&common.join("routes/topic.rs")));
        if topics.is_empty() {
            topics = str_consts(&module_block(&read_or_empty(&common.join("routes.rs")), "topic"));
        }

        Ok(Self {
            crates,
            files: rust.len(),
            lines,
            dependencies,
            config,
            topics,
        })
    }
}

fn read_or_empty(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

// The crates a manifest depends on at run time: its `[dependencies]`, the
// workspace's, and every target's.
fn dependency_names(manifest: &toml::Table) -> BTreeSet<String> {
    let mut names = table_keys(manifest.get("dependencies"));
    names.extend(table_keys(
        manifest.get("workspace").and_then(|workspace| workspace.get("dependencies")),
    ));
    for target in manifest.get("target").and_then(toml::Value::as_table).into_iter().flatten() {
        names.extend(table_keys(target.1.get("dependencies")));
    }
    names
}

// The string literal of every `const NAME: &str = "..."` line of `text`.
fn str_consts(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|line| line.contains("const ") && line.contains("&str"))
        .filter_map(|line| line.split_once("= \""))
        .filter_map(|(_, rest)| rest.split_once('"'))
        .map(|(literal, _)| literal.to_owned())
        .collect()
}

// The body of `mod <name> { ... }` in `text`, braces balanced; empty where
// the text declares no such module.
fn module_block(text: &str, name: &str) -> String {
    let Some(start) = text.find(&format!("mod {name}")) else {
        return String::new();
    };
    let Some(open) = text[start..].find('{') else {
        return String::new();
    };
    let body = &text[start + open + 1..];
    let mut depth = 1usize;
    for (offset, c) in body.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return body[..offset].to_owned();
                }
            }
            _ => {}
        }
    }
    body.to_owned()
}

// Every outbound URL the fixtures record is relative to this base; every
// endpoint the guest reads from its configuration is seeded to it.
const BASE_URL: &str = "http://api.test";

// The configuration the fixtures were recorded under: the reference's keys
// and the TypeScript source's, then every key of the built tree's catalog a
// pattern reads a value into, so a guest that spelled its own keys still
// finds its endpoints, identity, and window set.
fn config_seed(keys: &BTreeSet<String>) -> BTreeMap<String, String> {
    const FIXED: [(&str, &str); 11] = [
        ("ENV", "dev"),
        ("CC_STATIC_URL", BASE_URL),
        ("GTFS_CC_STATIC_URL", BASE_URL),
        ("BLOCK_MGT_URL", BASE_URL),
        ("BLOCK_MANAGEMENT_URL", BASE_URL),
        ("AZURE_IDENTITY", "block-mgt"),
        ("APP_MANIFEST_CLIENT_ID", "block-mgt"),
        ("IS_LOCAL", "true"),
        ("STATIONS", "0,19,40"),
        ("TIMEZONE", "Pacific/Auckland"),
        ("MAX_MESSAGE_DELAY_IN_SECONDS", "60"),
    ];
    let mut seed: BTreeMap<String, String> =
        FIXED.into_iter().map(|(key, value)| (key.to_owned(), value.to_owned())).collect();
    seed.insert("MIN_MESSAGE_DELAY_IN_SECONDS".to_owned(), "-30".to_owned());
    for key in keys {
        if !seed.contains_key(key)
            && let Some(value) = seed_for(key)
        {
            seed.insert(key.clone(), value.to_owned());
        }
    }
    seed
}

// The value a configuration key is seeded by its spelling: the first
// pattern its upper-cased name holds decides, a key no pattern reads left
// unset so the guest's own reading of a missing key shows.
fn seed_for(key: &str) -> Option<&'static str> {
    const PATTERNS: [(&[&str], &str); 13] = [
        (&["MAX_MESSAGE_DELAY", "MAX_DELAY"], "60"),
        (&["MIN_MESSAGE_DELAY", "MIN_DELAY"], "-30"),
        (&["SOURCE_TOPIC", "TOPIC_IN", "INBOUND_TOPIC", "CONSUME_TOPIC"], "realtime-train.v1"),
        (
            &["DEST_TOPIC", "DEST_VP_TOPIC", "TOPIC_OUT", "OUTBOUND_TOPIC", "PUBLISH_TOPIC"],
            "realtime-train-positions.v1",
        ),
        (&["URL", "HOST", "ENDPOINT", "BASE"], BASE_URL),
        (&["IDENTITY", "CLIENT_ID"], "block-mgt"),
        (&["STATION"], "0,19,40"),
        (&["TIMEZONE", "TIME_ZONE"], "Pacific/Auckland"),
        (&["DATE_FORMAT"], "DD/MM/YYYY"),
        (&["PREFIX"], ""),
        (&["DORMANT"], "false"),
        (&["LOCAL"], "true"),
        (&["KEY_VAULT", "VAULT"], "kv-test"),
    ];
    let upper = key.to_uppercase();
    if upper == "ENV" || upper == "ENVIRONMENT" {
        return Some("dev");
    }
    PATTERNS
        .iter()
        .find(|(patterns, _)| patterns.iter().any(|pattern| upper.contains(pattern)))
        .map(|(_, value)| *value)
}

// The topics the built guest may consume a fixture's message under, the
// likeliest first: every topic constant of its table and every topic its
// configuration was seeded with, bare and under the `dev-` prefix the
// layout's `topic_for` puts, then the TypeScript source's defaults.
fn inbound_candidates(topics: &BTreeSet<String>, config: &BTreeMap<String, String>) -> Vec<String> {
    let seeded = config
        .iter()
        .filter(|(key, _)| key.to_uppercase().contains("TOPIC"))
        .map(|(_, value)| value.as_str());
    let mut candidates: Vec<String> = Vec::new();
    for suffix in topics.iter().map(String::as_str).chain(seeded).chain(["realtime-train.v1"]) {
        if suffix.is_empty() {
            continue;
        }
        for candidate in [format!("dev-{suffix}"), suffix.to_owned()] {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    // the consumer's topic names the source before the destination
    let (train, rest): (Vec<String>, Vec<String>) = candidates
        .into_iter()
        .partition(|topic| topic.contains("train") && !topic.contains("position"));
    train.into_iter().chain(rest).collect()
}

// The built tree beside the reference: each measure on both sides, what
// the oracle found, and everything the harness printed as it ran.
pub struct Comparison {
    name: String,
    measures: Vec<Measure>,
    oracle: Oracle,
    log: String,
}

impl Comparison {
    // What `cargo test` printed as the harness ran, the guest's own events
    // among it; empty where the harness could not run.
    pub fn log(&self) -> &str {
        &self.log
    }

    // The one line the card's build findings carry.
    pub fn finding(&self) -> String {
        let measures: Vec<String> = self
            .measures
            .iter()
            .filter(|measure| {
                matches!(measure.name, "crates" | "Rust lines (non-blank)" | "dependencies")
            })
            .map(|measure| {
                let name = if measure.name == "Rust lines (non-blank)" {
                    "Rust lines"
                } else {
                    measure.name
                };
                format!("{name} {} vs {}", measure.built, measure.reference)
            })
            .collect();
        format!("reference oracle {}; {}", self.oracle.cell(), measures.join(", "))
    }
}

impl Display for Comparison {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\n## Reference comparison\n\nAgainst `{}`, the hand-written guest of the same \
             behaviour; a count is the tree's, a name one side alone spells.\n",
            self.name
        )?;
        writeln!(
            f,
            "| measure | built | reference | only built | only reference |\n| --- | --- | --- | \
             --- | --- |"
        )?;
        for measure in &self.measures {
            writeln!(
                f,
                "| {} | {} | {} | {} | {} |",
                measure.name,
                measure.built,
                measure.reference,
                spelled(&measure.only_built),
                spelled(&measure.only_reference)
            )?;
        }
        write!(f, "{}", self.oracle)
    }
}

// One measure on both sides: its count on each, and the names one side
// alone spells where the measure is of names.
struct Measure {
    name: &'static str,
    built: usize,
    reference: usize,
    only_built: Vec<String>,
    only_reference: Vec<String>,
}

impl Measure {
    const fn count(name: &'static str, built: usize, reference: usize) -> Self {
        Self {
            name,
            built,
            reference,
            only_built: Vec::new(),
            only_reference: Vec::new(),
        }
    }

    fn names(name: &'static str, built: &BTreeSet<String>, reference: &BTreeSet<String>) -> Self {
        Self {
            name,
            built: built.len(),
            reference: reference.len(),
            only_built: built.difference(reference).cloned().collect(),
            only_reference: reference.difference(built).cloned().collect(),
        }
    }
}

fn spelled(names: &[String]) -> String {
    let spelled: Vec<String> = names.iter().map(|name| format!("`{name}`")).collect();
    spelled.join(", ")
}

// What the harness answered: each fixture's outcome, the topic the guest
// consumed the messages under, the topics it published to, the table paths
// a fixture read that another fixture's recording answered, or why it ran
// nothing.
pub struct Oracle {
    outcomes: Vec<Outcome>,
    inbound: Option<String>,
    outbound: Vec<String>,
    shared: Vec<String>,
    note: Option<String>,
}

// One fixture driven through the guest: what it recorded, and every way
// the guest's answer differed, none for a pass.
struct Outcome {
    fixture: String,
    expected: String,
    reasons: Vec<String>,
}

impl Oracle {
    const fn not_run(why: String) -> Self {
        Self {
            outcomes: Vec::new(),
            inbound: None,
            outbound: Vec::new(),
            shared: Vec::new(),
            note: Some(why),
        }
    }

    // The harness's answer read from what `cargo test` printed; a run that
    // printed no outcome and named no cause, a compile failure, is noted
    // with the check's tail.
    fn read(check: &HostCheck, fixtures: usize) -> Self {
        let mut oracle = Self::parse(&check.output);
        if oracle.outcomes.is_empty() && oracle.note.is_none() {
            let exit =
                check.exit.map_or_else(|| "killed".to_owned(), |code| format!("exit {code}"));
            let tail = check.note.lines().last().unwrap_or("").trim();
            oracle.note = Some(format!("the harness answered nothing ({exit}): {tail}"));
        } else if oracle.outcomes.len() < fixtures && oracle.note.is_none() {
            oracle.note = Some(format!(
                "the harness answered {} of {fixtures} fixtures",
                oracle.outcomes.len()
            ));
        }
        oracle
    }

    // The lines the harness prints: `oracle <fixture> | <expected> | pass`,
    // `oracle <fixture> | <expected> | fail: <reason>; <reason>`,
    // `oracle-inbound <topic>`, `oracle-outbound <topic>, <topic>`,
    // `oracle-shared <path>, <path>`, and `oracle-none <why>`.
    fn parse(output: &str) -> Self {
        let mut oracle = Self {
            outcomes: Vec::new(),
            inbound: None,
            outbound: Vec::new(),
            shared: Vec::new(),
            note: None,
        };
        for line in output.lines().map(str::trim) {
            if let Some(rest) = marked(line, "oracle-inbound ") {
                oracle.inbound = Some(rest.to_owned());
            } else if let Some(rest) = marked(line, "oracle-outbound ") {
                oracle.outbound =
                    rest.split(", ").filter(|topic| !topic.is_empty()).map(str::to_owned).collect();
            } else if let Some(rest) = marked(line, "oracle-shared ") {
                oracle.shared =
                    rest.split(", ").filter(|path| !path.is_empty()).map(str::to_owned).collect();
            } else if let Some(rest) = marked(line, "oracle-none ") {
                oracle.note = Some(rest.to_owned());
            } else if let Some(rest) = marked(line, "oracle ") {
                let mut parts = rest.splitn(3, " | ");
                let (Some(fixture), Some(expected), Some(verdict)) =
                    (parts.next(), parts.next(), parts.next())
                else {
                    continue;
                };
                let reasons = verdict.strip_prefix("fail: ").map_or_else(Vec::new, |reasons| {
                    reasons.split("; ").map(str::to_owned).collect()
                });
                oracle.outcomes.push(Outcome {
                    fixture: fixture.to_owned(),
                    expected: expected.to_owned(),
                    reasons,
                });
            }
        }
        oracle
    }

    fn passed(&self) -> usize {
        self.outcomes.iter().filter(|outcome| outcome.reasons.is_empty()).count()
    }

    // The passes over the total per group, in the groups' order.
    fn groups(&self) -> Vec<(String, usize, usize)> {
        let mut groups: Vec<(String, usize, usize)> = Vec::new();
        for outcome in &self.outcomes {
            let group = outcome.fixture.split('/').next().unwrap_or("");
            let index = groups.iter().position(|(name, _, _)| name == group).unwrap_or_else(|| {
                groups.push((group.to_owned(), 0, 0));
                groups.len() - 1
            });
            groups[index].1 += usize::from(outcome.reasons.is_empty());
            groups[index].2 += 1;
        }
        groups
    }

    // The passes over the total, `not run` where the harness answered
    // nothing.
    fn cell(&self) -> String {
        if self.outcomes.is_empty() {
            return "not run".to_owned();
        }
        format!("{}/{}", self.passed(), self.outcomes.len())
    }
}

// The text after `marker` where the line opens with it, or where the test
// runner's `test <name> ... ` opened the line the harness printed first.
fn marked<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.strip_prefix(marker)
        .or_else(|| line.split_once("... ").and_then(|(_, rest)| rest.strip_prefix(marker)))
}

impl Display for Oracle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n### Oracle\n")?;
        if self.outcomes.is_empty() {
            return writeln!(
                f,
                "- not run: {}",
                self.note.as_deref().unwrap_or("the reference records no fixtures")
            );
        }
        let groups: Vec<String> = self
            .groups()
            .into_iter()
            .map(|(group, passed, total)| format!("{passed}/{total} {group}"))
            .collect();
        let inbound = self.inbound.as_deref().unwrap_or("none");
        let outbound =
            if self.outbound.is_empty() { "nothing".to_owned() } else { spelled(&self.outbound) };
        writeln!(
            f,
            "- {}/{} fixtures pass: {}; consumed under `{inbound}`, published to {outbound}.",
            self.passed(),
            self.outcomes.len(),
            groups.join(", ")
        )?;
        if let Some(note) = &self.note {
            writeln!(f, "- {note}")?;
        }
        if !self.shared.is_empty() {
            writeln!(
                f,
                "- {} answered from another fixture's recording where the fixture driven \
                 recorded no such read: a table read cache-aside on a miss.",
                spelled(&self.shared)
            )?;
        }
        writeln!(f, "\n| fixture | recorded | outcome |\n| --- | --- | --- |")?;
        for outcome in &self.outcomes {
            let verdict = if outcome.reasons.is_empty() {
                "pass".to_owned()
            } else {
                format!("fail: {}", outcome.reasons.join("; "))
            };
            writeln!(f, "| `{}` | {} | {verdict} |", outcome.fixture, outcome.expected)?;
        }
        Ok(())
    }
}

// The test the eval lays into the built tree as `tests/reference_oracle.rs`,
// `__CRATE__` the root package's crate name: every fixture under `oracle/`
// driven through the guest's messaging router over `omnia_test` doubles,
// its outcome printed in the lines `Oracle::parse` reads.
const HARNESS_SOURCE: &str = r#"//! The eval's reference oracle: every recorded fixture of the reference
//! guest driven through this guest's messaging router. Laid by the eval
//! into the exported tree and removed with it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use chrono::{Duration, Timelike, Utc};
use chrono_tz::Pacific::Auckland;
use http::{Method, Response};
use omnia_sdk::api::messaging::{Delivery, DeliveryError};
use omnia_test::guest::{FixedIdentity, MapConfig, MatchedHttp, Provider};
use serde_json::Value;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Metadata, Subscriber, span};

const BASE_URL: &str = "http://api.test";

// The longest a logged event is quoted at in a fixture's reasons.
const LOGGED_CHARS: usize = 240;

// The guest's warn and error events as one fixture runs, since a handler
// that logs a failure and answers `Ok(())` publishes nothing and says why
// here alone.
#[derive(Clone, Default)]
struct Logged(Arc<Mutex<Vec<String>>>);

impl Logged {
    fn last(&self) -> Option<String> {
        self.0.lock().expect("logged").last().cloned()
    }
}

impl Subscriber for Logged {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        *metadata.level() <= Level::WARN
    }

    fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let line = format!(
            "{} {}: {}",
            event.metadata().level(),
            event.metadata().target(),
            fields.0.trim()
        );
        self.0.lock().expect("logged").push(line);
    }

    fn enter(&self, _span: &span::Id) {}

    fn exit(&self, _span: &span::Id) {}
}

// An event's fields spelled as text, its message first.
#[derive(Default)]
struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.insert_str(0, &format!("{value:?} "));
        } else {
            let _ = write!(self.0, "{}={value:?} ", field.name());
        }
    }
}

// A logged line as a reason reads it: one line, within the quote length,
// with the separators the oracle's lines are split on spelled otherwise.
fn quoted(line: &str) -> String {
    let mut flat: String = line
        .replace('\n', " ")
        .replace(" | ", " / ")
        .replace("; ", ", ")
        .chars()
        .take(LOGGED_CHARS)
        .collect();
    if line.chars().count() > LOGGED_CHARS {
        flat.push('…');
    }
    flat
}

fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle")
}

fn delivery(topic: String, payload: Vec<u8>) -> Delivery {
    Delivery {
        topic: Some(topic),
        payload,
        ..Delivery::default()
    }
}

fn provider(config: &[(String, String)], http: MatchedHttp) -> Provider {
    Provider::default()
        .config(MapConfig::default().with(config.iter().cloned()))
        .http(http)
        .identity(FixedIdentity::new("test-token"))
}

fn panic_text(join: tokio::task::JoinError) -> String {
    match join.try_into_panic() {
        Ok(payload) => payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|text| (*text).to_owned()))
            .unwrap_or_else(|| "panicked".to_owned()),
        Err(join) => join.to_string(),
    }
}

#[tokio::test]
async fn reference_fixtures() {
    tokio::task::LocalSet::new().run_until(run()).await;
}

async fn run() {
    let dir = oracle_dir();
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("oracle.json")).expect("oracle.json"))
            .expect("oracle.json parses");
    let config: Vec<(String, String)> = settings["config"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, value)| (key.clone(), value.as_str().unwrap_or("").to_owned()))
        .collect();
    let candidates: Vec<String> = settings["inbound"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();

    // the inbound topic: the first candidate the router consumes
    let mut inbound = None;
    for topic in &candidates {
        let provider = provider(&config, MatchedHttp::default());
        let probed = topic.clone();
        let outcome = tokio::task::spawn_local(async move {
            let router = __CRATE__::messaging_router(provider).await;
            router.handle(delivery(probed, Vec::new())).await
        })
        .await;
        if !matches!(outcome, Ok(Err(DeliveryError::UnhandledTopic(_)))) {
            inbound = Some(topic.clone());
            break;
        }
    }
    let Some(inbound) = inbound else {
        println!("oracle-none no inbound topic consumed among: {}", candidates.join(", "));
        return;
    };
    println!("oracle-inbound {inbound}");

    // every fixture, in path order, over the recordings they share
    let mut fixtures: Vec<(String, Value)> = Vec::new();
    for group in ["static", "replay"] {
        let mut paths: Vec<PathBuf> = fs::read_dir(dir.join(group))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
            .collect();
        paths.sort();
        for path in paths {
            let stem = path.file_stem().map(|stem| stem.to_string_lossy().into_owned());
            let fixture: Value =
                serde_json::from_str(&fs::read_to_string(&path).expect("fixture")).expect("fixture parses");
            fixtures.push((format!("{group}/{}", stem.unwrap_or_default()), fixture));
        }
    }
    let shared = shared_recording(&fixtures);
    let mut outbound = BTreeSet::new();
    let mut answered_shared = BTreeSet::new();
    for (name, fixture) in &fixtures {
        let driven = drive(&inbound, &config, fixture, &shared).await;
        outbound.extend(driven.topics);
        answered_shared.extend(driven.shared);
        if driven.reasons.is_empty() {
            println!("oracle {name} | {} | pass", driven.expected);
        } else {
            println!("oracle {name} | {} | fail: {}", driven.expected, driven.reasons.join("; "));
        }
    }
    let outbound: Vec<String> = outbound.into_iter().collect();
    println!("oracle-outbound {}", outbound.join(", "));
    let answered_shared: Vec<String> = answered_shared.into_iter().collect();
    println!("oracle-shared {}", answered_shared.join(", "));
}

// The GET exchanges two or more fixtures record, by path and query, each
// under the first recording: the tables a guest reads cache-aside, which a
// fixture recorded over a warm cache never saw fetched.
fn shared_recording(fixtures: &[(String, Value)]) -> Vec<Value> {
    let mut seen: BTreeMap<String, (usize, Value)> = BTreeMap::new();
    for (_, fixture) in fixtures {
        let mut recorded = BTreeSet::new();
        for request in fixture["http_requests"].as_array().into_iter().flatten() {
            if request["method"].as_str().is_some_and(|method| method != "GET") {
                continue;
            }
            let key = request_key(request);
            if !recorded.insert(key.clone()) {
                continue;
            }
            let entry = seen.entry(key).or_insert_with(|| (0, request.clone()));
            entry.0 += 1;
        }
    }
    seen.into_values().filter(|(fixtures, _)| *fixtures >= 2).map(|(_, request)| request).collect()
}

fn request_key(request: &Value) -> String {
    let path = request["path"].as_str().unwrap_or("/");
    let query = request["request"].as_str().map(|query| format!("?{query}")).unwrap_or_default();
    format!("{path}{query}")
}

// Every URL `matched_http_into` answers the requests at: each with its
// query, and bare where it carries one.
fn urls_of(requests: &[&Value]) -> BTreeSet<String> {
    let mut urls = BTreeSet::new();
    for request in requests {
        let path = request["path"].as_str().unwrap_or("/");
        urls.insert(format!("{BASE_URL}{}", request_key(request)));
        urls.insert(format!("{BASE_URL}{path}"));
    }
    urls
}

// What one fixture driven through the guest showed.
struct Driven {
    expected: String,
    reasons: Vec<String>,
    topics: Vec<String>,
    shared: Vec<String>,
}

// One fixture through the guest: what it recorded, every difference in
// the guest's answer, the topics the guest published to, and the shared
// reads the fixture's own recording lacked that answered the guest.
async fn drive(inbound: &str, config: &[(String, String)], fixture: &Value, shared: &[Value]) -> Driven {
    let expected = Expected::of(fixture);
    let mut xml = fixture["input"].as_str().unwrap_or("").to_owned();
    if let Some(delay) = fixture["params"]["delay"].as_i64() {
        xml = shifted(&xml, delay);
    }
    let own: Vec<&Value> = fixture["http_requests"].as_array().into_iter().flatten().collect();
    let own_keys: BTreeSet<String> = own.iter().map(|request| request_key(request)).collect();
    let fallback: Vec<&Value> =
        shared.iter().filter(|request| !own_keys.contains(&request_key(request))).collect();
    let mut http = matched_http(&fixture["http_requests"]);
    for request in &fallback {
        http = matched_http_into(http, request);
    }
    let provider = provider(config, http);
    let sink = provider.publish.clone();
    let http = provider.http.clone();
    let logged = Logged::default();
    let topic = inbound.to_owned();
    let payload = xml.into_bytes();
    let outcome = {
        let _default = tracing::subscriber::set_default(logged.clone());
        tokio::task::spawn_local(async move {
            let router = __CRATE__::messaging_router(provider).await;
            router.handle(delivery(topic, payload)).await
        })
        .await
    };
    let sent = sink.sent();
    let topics: BTreeSet<String> = sent.iter().map(|(topic, _)| topic.clone()).collect();
    let own_urls = urls_of(&own);
    let answered: BTreeSet<String> = http
        .requests()
        .iter()
        .map(|request| request.uri.to_string())
        .filter(|url| urls_of(&fallback).contains(url) && !own_urls.contains(url))
        .map(|url| url.trim_start_matches(BASE_URL).to_owned())
        .collect();

    let mut reasons = BTreeSet::new();
    let result = match outcome {
        Ok(result) => result,
        Err(join) => {
            reasons.insert(format!("panicked: {}", panic_text(join).replace('\n', " ")));
            Err(DeliveryError::Rejected("panicked".to_owned()))
        }
    };
    match &expected {
        Expected::Refusal => {
            if result.is_ok() && !sent.is_empty() {
                reasons.insert(format!("published {} events where a refusal was recorded", sent.len()));
            }
        }
        Expected::Events(events) => {
            if let Err(error) = &result {
                reasons.insert(format!("refused: {}", error.to_string().replace('\n', " ")));
            }
            let published: Vec<Value> = sent
                .iter()
                .filter_map(|(_, message)| serde_json::from_slice(&message.payload).ok())
                .collect();
            if published.len() != sent.len() {
                reasons.insert(format!("{} of {} payloads are not JSON", sent.len() - published.len(), sent.len()));
            }
            // the events compared in sorted order, the distinct ones where
            // the counts differ, so a guest publishing each once is still
            // told what its events hold
            let mut recorded = events.clone();
            recorded.sort_by_key(event_key);
            let mut got = published.clone();
            got.sort_by_key(event_key);
            if got.len() != recorded.len() {
                reasons.insert(format!("published {} events, {} recorded", got.len(), recorded.len()));
                recorded.dedup_by_key(|event| event_key(event));
                got.dedup_by_key(|event| event_key(event));
            }
            if got.len() == recorded.len() {
                for (want, have) in recorded.iter().zip(&got) {
                    compare_event(want, have, &mut reasons);
                }
            } else {
                reasons.insert(format!(
                    "{} distinct events published, {} recorded",
                    got.len(),
                    recorded.len()
                ));
            }
            for (_, message) in &sent {
                let event: Option<Value> = serde_json::from_slice(&message.payload).ok();
                let id = event.as_ref().and_then(|event| text_at(event, &["remoteData", "externalId"]));
                match (message.headers.get("key"), id) {
                    (None, Some(id)) => {
                        reasons.insert(format!("no `key` header on the message for `{id}`"));
                    }
                    (Some(key), Some(id)) if *key != id => {
                        reasons.insert(format!("`key` header `{key}` on the message for `{id}`"));
                    }
                    _ => {}
                }
            }
        }
    }
    // what the guest said as it failed, since a handler that logs and
    // answers `Ok(())` is otherwise silent
    if !reasons.is_empty()
        && let Some(last) = logged.last()
    {
        reasons.insert(format!("logged: {}", quoted(&last)));
    }
    Driven {
        expected: expected.label(),
        reasons: reasons.into_iter().collect(),
        topics: topics.into_iter().collect(),
        shared: answered.into_iter().collect(),
    }
}

enum Expected {
    Refusal,
    Events(Vec<Value>),
}

impl Expected {
    fn of(fixture: &Value) -> Self {
        let output = &fixture["output"];
        if output.get("failure").is_some() {
            return Self::Refusal;
        }
        Self::Events(output["success"].as_array().cloned().unwrap_or_default())
    }

    fn label(&self) -> String {
        match self {
            Self::Refusal => "a refusal".to_owned(),
            Self::Events(events) => format!("{} events", events.len()),
        }
    }
}

// A field of an event under its camelCase path, or the snake_case
// spelling of each segment.
fn at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        let snake = snake_case(segment);
        current = current.get(segment).or_else(|| current.get(&snake))?;
    }
    Some(current)
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_uppercase() {
            out.push('_');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn text_at(value: &Value, path: &[&str]) -> Option<String> {
    at(value, path).map(|field| match field {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    })
}

fn number_at(value: &Value, path: &[&str]) -> Option<f64> {
    at(value, path).and_then(|field| match field {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    })
}

fn event_key(event: &Value) -> (String, String, String) {
    let coordinate = |path: &[&str]| number_at(event, path).map(|n| format!("{n:.6}")).unwrap_or_default();
    (
        text_at(event, &["remoteData", "externalId"]).unwrap_or_default(),
        coordinate(&["locationData", "latitude"]),
        coordinate(&["locationData", "longitude"]),
    )
}

fn compare_event(want: &Value, have: &Value, reasons: &mut BTreeSet<String>) {
    let id = text_at(want, &["remoteData", "externalId"]).unwrap_or_default();
    for path in [&["eventType"][..], &["remoteData", "externalId"][..]] {
        let wanted = text_at(want, path).unwrap_or_default();
        let got = text_at(have, path);
        if got.as_deref() != Some(wanted.as_str()) {
            reasons.insert(format!(
                "`{}` is {} for `{id}`, `{wanted}` recorded",
                path.join("."),
                got.map_or_else(|| "absent".to_owned(), |got| format!("`{got}`"))
            ));
        }
    }
    for field in ["latitude", "longitude", "speed", "gpsAccuracy"] {
        let path = ["locationData", field];
        let wanted = number_at(want, &path);
        let got = number_at(have, &path);
        let agrees = match (wanted, got) {
            (Some(wanted), Some(got)) => (wanted - got).abs() < 1e-6,
            (None, None) => true,
            _ => false,
        };
        if !agrees {
            reasons.insert(format!(
                "`locationData.{field}` is {} for `{id}`, {} recorded",
                got.map_or_else(|| "absent".to_owned(), |got| got.to_string()),
                wanted.map_or_else(|| "absent".to_owned(), |wanted| wanted.to_string())
            ));
        }
    }
}

// The fixture's recorded requests answered at their exact URL, at the bare
// path, and, for a trip allocation answered as labels, at the URL and in
// the shape the TypeScript source reads.
fn matched_http(requests: &Value) -> MatchedHttp {
    let mut http = MatchedHttp::default();
    for request in requests.as_array().into_iter().flatten() {
        http = matched_http_into(http, request);
    }
    http
}

fn matched_http_into(http: MatchedHttp, request: &Value) -> MatchedHttp {
    let mut http = http;
    let method = request["method"].as_str().unwrap_or("GET");
    let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
    let path = request["path"].as_str().unwrap_or("/");
    let query = request["request"].as_str().map(|query| format!("?{query}")).unwrap_or_default();
    let status = request["response"]["status"].as_u64().unwrap_or(200) as u16;
    let body = &request["response"]["body"];
    let answer = |body: &Value| {
        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Bytes::from(body.to_string()))
            .expect("fixture response")
    };
    http = http.on(method.clone(), format!("{BASE_URL}{path}{query}"), answer(body));
    if !query.is_empty() {
        http = http.on(method.clone(), format!("{BASE_URL}{path}"), answer(body));
    }
    if path == "/allocations/trips"
        && let Some(labels) = body.as_array()
        && labels.iter().all(Value::is_string)
    {
        let all: Vec<Value> = labels
            .iter()
            .map(|label| serde_json::json!({ "vehicleLabel": label }))
            .collect();
        let shaped = serde_json::json!({ "all": all });
        http = http.on(
            method,
            format!("{BASE_URL}{path}{query}&closestTrip=true&include_filtered_routes=true"),
            answer(&shaped),
        );
    }
    http
}

// The message's creation date and its first change's actual time
// re-anchored to `delay` seconds before now in the source's timezone, as
// the reference's fixture loader re-anchors them.
fn shifted(xml: &str, delay: i64) -> String {
    let event = Utc::now().with_timezone(&Auckland) - Duration::seconds(delay);
    let date = event.format("%d/%m/%Y").to_string();
    let seconds = event.num_seconds_from_midnight().to_string();
    let mut xml = with_text(xml, "fechaCreacion", &date);
    if let Some((start, end)) = inner_span(&xml, "pasoTren") {
        let change = xml[start..end].to_owned();
        let departed = text_of(&change, "haSalido") == Some("true");
        let arrived = text_of(&change, "haEntrado") == Some("true");
        let edited = if departed {
            with_text(&change, "horaSalidaReal", &seconds)
        } else if arrived {
            with_text(&change, "horaEntradaReal", &seconds)
        } else {
            change
        };
        xml.replace_range(start..end, &edited);
    }
    xml
}

fn inner_span(xml: &str, element: &str) -> Option<(usize, usize)> {
    let open = format!("<{element}>");
    let close = format!("</{element}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&close)?;
    Some((start, end))
}

fn text_of<'a>(xml: &'a str, element: &str) -> Option<&'a str> {
    inner_span(xml, element).map(|(start, end)| &xml[start..end])
}

fn with_text(xml: &str, element: &str, text: &str) -> String {
    match inner_span(xml, element) {
        Some((start, end)) => format!("{}{text}{}", &xml[..start], &xml[end..]),
        None => xml.to_owned(),
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_lines() {
        let output = "\
   Compiling guest v0.1.0
test reference_fixtures ... oracle-inbound dev-realtime-train.v1
oracle static/0001 | 2 events | pass
oracle static/0006 | a refusal | fail: published 2 events where a refusal was recorded
oracle replay/0304 | 4 events | fail: refused: config key `X` is not set; published 0 events, 4 recorded
oracle-outbound dev-realtime-train-positions.v1
test reference_fixtures ... ok
";
        let oracle = Oracle::parse(output);
        assert_eq!(oracle.inbound.as_deref(), Some("dev-realtime-train.v1"));
        assert_eq!(oracle.outbound, ["dev-realtime-train-positions.v1"]);
        assert_eq!(oracle.passed(), 1);
        assert_eq!(oracle.cell(), "1/3");
        assert_eq!(oracle.groups(), [("static".to_owned(), 1, 2), ("replay".to_owned(), 0, 1)]);
        assert_eq!(oracle.outcomes[2].reasons.len(), 2);
        assert_eq!(oracle.outcomes[1].expected, "a refusal");
        let text = oracle.to_string();
        assert!(text.contains("- 1/3 fixtures pass: 1/2 static, 0/1 replay"), "{text}");
        assert!(text.contains("| `static/0006` | a refusal | fail: published 2 events"), "{text}");
    }

    #[test]
    fn oracle_without_a_topic() {
        let oracle = Oracle::parse("oracle-none no inbound topic consumed among: dev-a, a\n");
        assert_eq!(oracle.cell(), "not run");
        assert!(
            oracle.to_string().contains("- not run: no inbound topic consumed among: dev-a, a")
        );
    }

    #[test]
    fn oracle_harness_silent() {
        let check = HostCheck {
            name: "cargo test",
            exit: Some(101),
            output: "error[E0433]: failed to resolve\n".to_owned(),
            note: "error[E0433]: failed to resolve".to_owned(),
        };
        let oracle = Oracle::read(&check, 22);
        assert_eq!(
            oracle.note.as_deref(),
            Some("the harness answered nothing (exit 101): error[E0433]: failed to resolve")
        );
        assert_eq!(oracle.cell(), "not run");
    }

    #[test]
    fn str_consts_of_a_catalog() {
        let text = "pub const ENV: &str = \"ENV\";\n/// doc\npub const URL: &str = \"BLOCK_MGT_URL\";\npub const N: u32 = 3;\n";
        let consts = str_consts(text);
        assert_eq!(consts.into_iter().collect::<Vec<_>>(), ["BLOCK_MGT_URL", "ENV"]);
    }

    #[test]
    fn topic_module_block() {
        let text = "pub mod http {}\n\npub mod topic {\n    pub const TRAIN: &str = \"realtime-train.v1\";\n    pub mod inner { pub const X: &str = \"x\"; }\n}\npub const OUT: &str = \"outside\";\n";
        let topics = str_consts(&module_block(text, "topic"));
        assert_eq!(topics.into_iter().collect::<Vec<_>>(), ["realtime-train.v1", "x"]);
        assert_eq!(module_block(text, "absent"), "");
    }

    #[test]
    fn config_seeded_by_pattern() {
        let keys: BTreeSet<String> = [
            "ENV",
            "GTFS_URL",
            "BLOCK_MGT_IDENTITY",
            "STATION_FILTER",
            "UNRELATED",
            "KAFKA_SOURCE_TOPIC",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let seed = config_seed(&keys);
        assert_eq!(seed["ENV"], "dev");
        assert_eq!(seed["GTFS_URL"], BASE_URL);
        assert_eq!(seed["BLOCK_MGT_IDENTITY"], "block-mgt");
        assert_eq!(seed["STATION_FILTER"], "0,19,40");
        assert_eq!(seed["KAFKA_SOURCE_TOPIC"], "realtime-train.v1");
        assert_eq!(seed["MIN_MESSAGE_DELAY_IN_SECONDS"], "-30");
        assert!(!seed.contains_key("UNRELATED"));
    }

    #[test]
    fn inbound_candidates_lead_with_the_source() {
        let topics: BTreeSet<String> = ["realtime-train-positions.v1", "realtime-train.v1"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let config = config_seed(&BTreeSet::new());
        let candidates = inbound_candidates(&topics, &config);
        assert_eq!(candidates[0], "dev-realtime-train.v1");
        assert_eq!(candidates[1], "realtime-train.v1");
        assert!(candidates.contains(&"dev-realtime-train-positions.v1".to_owned()));
        let distinct: BTreeSet<&String> = candidates.iter().collect();
        assert_eq!(distinct.len(), candidates.len());
    }

    #[test]
    fn dev_dependencies_added_once() {
        let manifest: toml::Table = "\
[package]
name = \"guest\"
[workspace.dependencies]
chrono = \"0.4\"
serde_json = \"1\"
[target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies]
omnia-test = { workspace = true }
tokio = { workspace = true }
serde_json.workspace = true
"
        .parse()
        .unwrap();
        let additions = dev_dependency_additions(&manifest).unwrap();
        assert!(additions.contains(".dev-dependencies.chrono]\nworkspace = true"), "{additions}");
        assert!(
            additions.contains(".dev-dependencies.chrono-tz]\nversion = \"0.10\""),
            "{additions}"
        );
        assert!(!additions.contains("serde_json"), "{additions}");
        let appended: toml::Table =
            format!("{}\n{additions}", toml::to_string(&manifest).unwrap()).parse().unwrap();
        assert_eq!(dev_dependency_names(&appended).len(), 7);

        let bare: toml::Table = "[package]\nname = \"guest\"\n".parse().unwrap();
        assert_eq!(
            dev_dependency_additions(&bare).unwrap_err(),
            "the built tree's root package has no `omnia-test` dev-dependency"
        );
    }

    #[test]
    fn comparison_finding() {
        let built: BTreeSet<String> =
            ["common", "train-update"].into_iter().map(str::to_owned).collect();
        let reference: BTreeSet<String> =
            ["common", "position-adapter"].into_iter().map(str::to_owned).collect();
        let comparison = Comparison {
            name: "evals/cases/train/reference".to_owned(),
            measures: vec![
                Measure::names("crates", &built, &reference),
                Measure::count("Rust lines (non-blank)", 900, 1200),
                Measure::names("dependencies", &built, &built),
            ],
            oracle: Oracle::parse("oracle static/0001 | 2 events | pass\n"),
        };
        assert_eq!(
            comparison.finding(),
            "reference oracle 1/1; crates 2 vs 2, Rust lines 900 vs 1200, dependencies 2 vs 2"
        );
        let text = comparison.to_string();
        assert!(
            text.contains("| crates | 2 | 2 | `train-update` | `position-adapter` |"),
            "{text}"
        );
        assert!(text.contains("| Rust lines (non-blank) | 900 | 1200 |  |  |"), "{text}");
    }
}

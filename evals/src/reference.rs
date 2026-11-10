//! Compares `omnia` builds against a case's reference: a vendored guest with its
//! replay fixtures, or the source's behaviour recorded as HTTP scenarios.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};
use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fidelity::{HostCheck, files_under, host_check};

// The reference a case names, located under the repository root.
pub struct Reference {
    dir: PathBuf,
    name: String,
    kind: Kind,
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
        let kind = match fs::read_to_string(dir.join(SPEC)) {
            Ok(text) => match Spec::parse(&text) {
                Ok(spec) => Kind::Scenarios(spec),
                Err(why) => {
                    eprintln!("eval: reference `{name}`: {SPEC} {why}; nothing compared");
                    return None;
                }
            },
            Err(_) => Kind::Guest,
        };
        Some(Self {
            dir,
            name: name.to_owned(),
            kind,
        })
    }

    // The built tree at `export` measured against a reference guest, and
    // the reference's fixtures or scenarios driven through the built guest.
    pub fn compare(&self, export: &Path) -> io::Result<Comparison> {
        let built = Shape::read(export)?;
        let (measures, staged) = match &self.kind {
            Kind::Guest => {
                let reference = Shape::read(&self.dir)?;
                (built.against(&reference), self.stage(export, &built)?)
            }
            Kind::Scenarios(spec) => (Vec::new(), self.stage_scenarios(export, &built, spec)?),
        };
        let (oracle, log) = match staged {
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

    // The harness written into the export: the fixtures copied under
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
        let manifest = match Manifest::read(export, &[])? {
            Ok(manifest) => manifest,
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

        manifest.write_harness(export, HARNESS_SOURCE)?;
        Ok(Ok(fixtures.len()))
    }

    // Every recorded scenario of the reference, as `<group>/<name>` and its
    // JSON, in path order: `scenarios/<group>/<name>.json`.
    fn scenarios(&self) -> io::Result<Vec<(String, Value)>> {
        let mut found = Vec::new();
        for group in fs::read_dir(self.dir.join("scenarios")).into_iter().flatten() {
            let group = group?.path();
            if !group.is_dir() {
                continue;
            }
            let group_name = group.file_name().map(|name| name.to_string_lossy().into_owned());
            for entry in fs::read_dir(&group)? {
                let path = entry?.path();
                if path.extension().is_some_and(|extension| extension == "json")
                    && let Some(stem) = path.file_stem()
                {
                    let scenario: Value = serde_json::from_str(&fs::read_to_string(&path)?)
                        .map_err(|error| {
                            io::Error::other(format!("{}: {error}", path.display()))
                        })?;
                    found.push((
                        format!(
                            "{}/{}",
                            group_name.clone().unwrap_or_default(),
                            stem.to_string_lossy()
                        ),
                        scenario,
                    ));
                }
            }
        }
        found.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(found)
    }

    // The scenario harness written into the export: `oracle/oracle.json`
    // carrying the scenarios, the schema and seeds of the store, the
    // upstream answers, and the configuration, with the dev-dependencies
    // the harness reads and the test itself; `Err` names why it cannot run.
    fn stage_scenarios(
        &self, export: &Path, built: &Shape, spec: &Spec,
    ) -> io::Result<Result<usize, String>> {
        let scenarios = self.scenarios()?;
        if scenarios.is_empty() {
            return Ok(Err(format!(
                "`{}` holds no scenarios under `scenarios/<group>/`",
                self.name
            )));
        }
        for (name, scenario) in &scenarios {
            if let Err(why) = check_scenario(scenario) {
                return Ok(Err(format!("scenario `{name}` {why}")));
            }
        }
        let manifest = match Manifest::read(export, &SCENARIO_DEPENDENCIES)? {
            Ok(manifest) => manifest,
            Err(why) => return Ok(Err(why)),
        };

        // the store's schema and seeds, and the upstream's default answers
        let schema = read_or_empty(&self.dir.join("schema.sql"));
        let mut tables = serde_json::Map::new();
        for entry in fs::read_dir(self.dir.join("seed/tables")).into_iter().flatten() {
            let path = entry?.path();
            if path.extension().is_some_and(|extension| extension == "json")
                && let Some(table) = path.file_stem()
            {
                let rows: Value = serde_json::from_str(&fs::read_to_string(&path)?)
                    .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
                tables.insert(table.to_string_lossy().into_owned(), rows);
            }
        }
        let http: Value = match fs::read_to_string(self.dir.join("seed/http.json")) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|error| io::Error::other(format!("seed/http.json: {error}")))?,
            Err(_) => Value::Object(serde_json::Map::new()),
        };

        // the settings
        let oracle = export.join("oracle");
        fs::create_dir_all(&oracle)?;
        let settings = serde_json::json!({
            "config": scenario_config(&spec.config, &spec.auth.token, &built.config),
            "auth": spec.auth,
            "schema": schema,
            "tables": tables,
            "http": http,
            "scenarios": scenarios
                .iter()
                .map(|(name, scenario)| {
                    serde_json::json!({
                        "name": name,
                        "seed": scenario.get("seed").cloned().unwrap_or(Value::Null),
                        "steps": scenario["steps"],
                    })
                })
                .collect::<Vec<Value>>(),
        });
        fs::write(oracle.join("oracle.json"), serde_json::to_string_pretty(&settings)?)?;

        manifest.write_harness(export, SCENARIO_HARNESS_SOURCE)?;
        Ok(Ok(scenarios.len()))
    }
}
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
        let oracle = format!("reference oracle {}", self.oracle.cell());
        if measures.is_empty() { oracle } else { format!("{oracle}; {}", measures.join(", ")) }
    }
}

impl Display for Comparison {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.measures.is_empty() {
            writeln!(
                f,
                "\n## Reference comparison\n\nAgainst `{}`, the source's behaviour recorded as \
                 scenarios, each driven through the built guest over one fresh store.",
                self.name
            )?;
            return write!(f, "{}", self.oracle);
        }
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
enum Kind {
    Guest,
    Scenarios(Spec),
}

// What `oracle.toml` declares over a reference of recorded scenarios: the
// bearer its guarded paths take, and the configuration the scenarios were
// recorded under.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    kind: String,
    auth: Auth,
    #[serde(default)]
    config: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Auth {
    header: String,
    token: String,
    guarded: String,
}

impl Spec {
    fn parse(text: &str) -> Result<Self, String> {
        let spec: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        if spec.kind != "http" {
            return Err(format!("declares `kind = \"{}\"`, and `http` is the one kind", spec.kind));
        }
        if !spec.auth.guarded.starts_with('/') {
            return Err("declares a guarded prefix that is no path".to_owned());
        }
        Ok(spec)
    }
}

// What a scenario must hold to be driven: steps, each one request with its
// expected status or one statement, and every `{name}` a step spells
// captured by an earlier one or one of the harness's own placeholders.
fn check_scenario(scenario: &Value) -> Result<(), String> {
    const PLACEHOLDERS: [&str; 3] = ["uuid", "timestamp", "any"];
    let Some(steps) = scenario["steps"].as_array().filter(|steps| !steps.is_empty()) else {
        return Err("has no `steps`".to_owned());
    };
    for entry in scenario["seed"]["http"].as_array().into_iter().flatten() {
        if entry["path"].as_str().is_none_or(|path| !path.starts_with('/')) {
            return Err("seeds an upstream answer with no `path`".to_owned());
        }
    }
    let mut captured: BTreeSet<String> =
        PLACEHOLDERS.iter().map(|name| (*name).to_owned()).collect();
    for (index, step) in steps.iter().enumerate() {
        let at = index + 1;
        match (step.get("request"), step.get("sql")) {
            (Some(request), None) => {
                if request["method"].as_str().is_none() {
                    return Err(format!("step {at} has no `request.method`"));
                }
                if request["path"].as_str().is_none_or(|path| !path.starts_with('/')) {
                    return Err(format!("step {at} has no `request.path`"));
                }
                if step["expect"]["status"].as_u64().is_none() {
                    return Err(format!("step {at} has no `expect.status`"));
                }
                let mut spelled = BTreeSet::new();
                placeholders(request, &mut spelled);
                for (name, path) in step["capture"].as_object().into_iter().flatten() {
                    if path.as_str().is_none() {
                        return Err(format!("step {at} captures `{name}` from no path"));
                    }
                    captured.insert(name.clone());
                }
                placeholders(&step["expect"], &mut spelled);
                if let Some(unknown) = spelled.iter().find(|name| !captured.contains(*name)) {
                    return Err(format!(
                        "step {at} spells `{{{unknown}}}`, which no earlier step captures"
                    ));
                }
            }
            (None, Some(sql)) => {
                let Some(sql) = sql.as_str() else {
                    return Err(format!("step {at} has a `sql` that is no string"));
                };
                let mut spelled = BTreeSet::new();
                placeholders(&Value::String(sql.to_owned()), &mut spelled);
                if let Some(unknown) = spelled.iter().find(|name| !captured.contains(*name)) {
                    return Err(format!(
                        "step {at} spells `{{{unknown}}}`, which no earlier step captures"
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "step {at} has one of `request` and `sql`, not both or neither"
                ));
            }
        }
    }
    Ok(())
}

// Every `{name}` the strings of `value` spell, at any depth.
fn placeholders(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            let mut rest = text.as_str();
            while let Some(open) = rest.find('{') {
                let Some(close) = rest[open..].find('}') else { break };
                let name = &rest[open + 1..open + close];
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    names.insert(name.to_owned());
                }
                rest = &rest[open + close + 1..];
            }
        }
        Value::Array(items) => items.iter().for_each(|item| placeholders(item, names)),
        Value::Object(fields) => fields.values().for_each(|field| placeholders(field, names)),
        _ => {}
    }
}

// The configuration a scenarios reference seeds the guest with: the
// reference's table whole, then every key of the built tree's settings the
// table lacks, by its longest `_`-suffix in the table (`ORDERS_GST_RATE`
// from `GST_RATE`), else by the generic pattern its name holds; a key
// neither reads is left unset so the guest's own reading of it shows.
fn scenario_config(
    table: &BTreeMap<String, String>, token: &str, keys: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut seed = table.clone();
    for key in keys {
        if seed.contains_key(key) {
            continue;
        }
        let upper = key.to_uppercase();
        let value = table
            .get(&upper)
            .cloned()
            .or_else(|| {
                upper.match_indices('_').find_map(|(at, _)| table.get(&upper[at + 1..]).cloned())
            })
            .or_else(|| generic_seed(&upper, token));
        if let Some(value) = value {
            seed.insert(key.clone(), value);
        }
    }
    seed
}

// The value a key of no recorded name is seeded by the pattern its
// upper-cased name holds: an endpoint reaches the harness's upstream, a
// token is the bearer, and a window or a size is a plausible default.
fn generic_seed(upper: &str, token: &str) -> Option<String> {
    const PATTERNS: [(&[&str], &str); 8] = [
        (&["URL", "HOST", "ENDPOINT", "BASE"], BASE_URL),
        (&["API_KEY", "SECRET", "PASSWORD"], "secret"),
        (&["TTL", "SECONDS", "TIMEOUT", "INTERVAL"], "60"),
        (&["PAGE_SIZE", "LIMIT"], "20"),
        (&["PORT"], "3000"),
        (&["LEVEL"], "info"),
        (&["PREFIX"], ""),
        (&["ENV"], "dev"),
    ];
    if upper.contains("TOKEN") {
        return Some(token.to_owned());
    }
    PATTERNS
        .iter()
        .find(|(patterns, _)| patterns.iter().any(|pattern| upper.contains(pattern)))
        .map(|(_, value)| (*value).to_owned())
}

const GROUPS: [&str; 2] = ["static", "replay"];

const HARNESS: &str = "reference_oracle";

const SPEC: &str = "oracle.toml";

fn harness_path() -> String {
    format!("tests/{HARNESS}.rs")
}

// The built tree's root manifest as the harness is staged into it: the
// package whose router the harness drives, and the dev-dependencies to
// append for the harness to compile.
struct Manifest {
    path: PathBuf,
    text: String,
    package: String,
    additions: String,
}

impl Manifest {
    // The root manifest read with the additions `extra` and the shared list
    // need; `Err` names why no harness can run over it.
    fn read(export: &Path, extra: &[Dependency]) -> io::Result<Result<Self, String>> {
        let path = export.join("Cargo.toml");
        let text = fs::read_to_string(&path)?;
        let table: toml::Table = text
            .parse()
            .map_err(|error| io::Error::other(format!("the built tree's Cargo.toml: {error}")))?;
        let Some(package) = table
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
        else {
            return Ok(Err("the built tree's root `Cargo.toml` declares no package".to_owned()));
        };
        let additions = match dev_dependency_additions(&table, extra) {
            Ok(additions) => additions,
            Err(why) => return Ok(Err(why)),
        };
        Ok(Ok(Self {
            path,
            text,
            package: package.to_owned(),
            additions,
        }))
    }

    // The harness `source` written as the root package's test, over the
    // dev-dependencies appended to the manifest.
    fn write_harness(&self, export: &Path, source: &str) -> io::Result<()> {
        if !self.additions.is_empty() {
            fs::write(&self.path, format!("{}\n{}", self.text, self.additions))?;
        }
        fs::create_dir_all(export.join("tests"))?;
        fs::write(
            export.join(harness_path()),
            source.replace("__CRATE__", &self.package.replace('-', "_")),
        )
    }
}

// A crate a harness reads: its name, the version taken where the workspace
// pins none, and the features it needs.
struct Dependency {
    name: &'static str,
    version: &'static str,
    features: &'static [&'static str],
}

impl Dependency {
    const fn plain(name: &'static str, version: &'static str) -> Self {
        Self {
            name,
            version,
            features: &[],
        }
    }
}

// What every harness reads.
const SHARED_DEPENDENCIES: [Dependency; 6] = [
    Dependency::plain("bytes", "1"),
    Dependency::plain("chrono", "0.4"),
    Dependency::plain("chrono-tz", "0.10"),
    Dependency::plain("http", "1"),
    Dependency::plain("serde_json", "1"),
    Dependency::plain("tracing", "0.1"),
];

// What the scenario harness reads besides: the in-memory database its
// table store runs the guest's statements against, the declared column
// types read back through `column_decltype`, and `now()` registered
// through `functions`.
const SCENARIO_DEPENDENCIES: [Dependency; 1] = [Dependency {
    name: "rusqlite",
    version: "0.41",
    features: &["bundled", "column_decltype", "functions"],
}];

// The TOML the harness's dev-dependencies are appended to the built tree's
// root manifest as, one table per crate the root package depends on under
// no table: the workspace's pin where it carries one, a version otherwise.
// `omnia-test` and `tokio` are the layout's own and are never added, since
// their versions and features are the tree's to pin.
fn dev_dependency_additions(
    manifest: &toml::Table, extra: &[Dependency],
) -> Result<String, String> {
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
    for dependency in SHARED_DEPENDENCIES.iter().chain(extra) {
        if present.contains(dependency.name) {
            continue;
        }
        let mut entry = if workspace.contains(dependency.name) {
            "workspace = true".to_owned()
        } else {
            format!("version = \"{}\"", dependency.version)
        };
        if !dependency.features.is_empty() {
            let features: Vec<String> =
                dependency.features.iter().map(|feature| format!("\"{feature}\"")).collect();
            let _ = write!(entry, "\nfeatures = [{}]", features.join(", "));
        }
        let _ = write!(additions, "[{NATIVE}.{}]\n{entry}\n\n", dependency.name);
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
        for target in manifest.get("target").and_then(toml::Value::as_table).into_iter().flatten() {
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

        // the settings: the catalog where a tree keeps one, the per-part
        // constants spelled as their own names, and the keys of each part's
        // settings struct
        let common = root.join("crates/common/src");
        let mut config = str_consts(&read_or_empty(&common.join("config.rs")));
        for path in rust.iter().filter(|path| path.starts_with("crates/")) {
            let text = fs::read_to_string(root.join(path))?;
            config.extend(setting_consts(&text));
            config.extend(setting_fields(&text));
        }

        // the topic table, wherever the tree's shape keeps it
        let mut topics = str_consts(&read_or_empty(&common.join("topic.rs")));
        if topics.is_empty() {
            topics = str_consts(&read_or_empty(&common.join("routes/topic.rs")));
        }
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

    // Each measure of this tree beside the reference guest's.
    fn against(&self, reference: &Self) -> Vec<Measure> {
        vec![
            Measure::names("crates", &self.crates, &reference.crates),
            Measure::count("Rust files", self.files, reference.files),
            Measure::count("Rust lines (non-blank)", self.lines, reference.lines),
            Measure::names("dependencies", &self.dependencies, &reference.dependencies),
            Measure::names("config keys", &self.config, &reference.config),
            Measure::names("topics", &self.topics, &reference.topics),
        ]
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

// The name of every `const NAME: &str = "NAME";` line of `text`: a setting
// as a part declares it, its value its own name.
fn setting_consts(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.split_once("const "))
        .filter_map(|(_, rest)| rest.split_once(':'))
        .filter_map(|(name, rest)| {
            let literal = rest.split_once("= \"")?.1.split_once('"')?.0;
            (literal == name.trim()).then(|| literal.to_owned())
        })
        .collect()
}

// The key of every field of a `#[derive(.., Settings)]` struct of `text`,
// as the derive reads it: the field's name upper-cased behind the struct's
// `#[setting(prefix = "..")]`, or the field's own `#[setting(key = "..")]`.
fn setting_fields(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("#[derive(") {
        let start = from + at + "#[derive(".len();
        let Some(close) = text[start..].find(")]") else { break };
        from = start + close;
        if !text[start..from].split(',').any(|name| name.trim() == "Settings") {
            continue;
        }
        let Some(open) = text[from..].find('{') else { break };
        let Some(end) = text[from + open..].find('}') else { break };
        let prefix = setting_attribute(&text[from..from + open], "prefix").unwrap_or_default();
        let mut explicit = None;
        for line in text[from + open + 1..from + open + end].lines().map(str::trim) {
            if let Some(key) = setting_attribute(line, "key") {
                explicit = Some(key);
            } else if !line.starts_with("//")
                && !line.starts_with('#')
                && let Some((field, _)) = line.split_once(':')
            {
                let field = field.rsplit(' ').next().unwrap_or(field);
                keys.insert(
                    explicit
                        .take()
                        .unwrap_or_else(|| format!("{prefix}{}", field.to_ascii_uppercase())),
                );
            }
        }
        from += open + end;
    }
    keys
}

// The string a `#[setting(<name> = "..")]` attribute in `text` spells.
fn setting_attribute(text: &str, name: &str) -> Option<String> {
    let lead = format!("{name} = \"");
    let at = text.find(&lead)? + lead.len();
    text[at..].split_once('"').map(|(value, _)| value.to_owned())
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

// What the harness answered: each fixture's or scenario's outcome, the topic
// the guest consumed the messages under, the topics it published to, the
// table paths a fixture read that another fixture's recording answered, or
// why it ran nothing.
struct Oracle {
    outcomes: Vec<Outcome>,
    // `scenarios` where the harness drove recorded requests, else fixtures
    kind: Option<String>,
    inbound: Option<String>,
    outbound: Vec<String>,
    shared: Vec<String>,
    note: Option<String>,
}

// One fixture or scenario driven through the guest: what it recorded, and
// every way the guest's answer differed, none for a pass.
struct Outcome {
    fixture: String,
    expected: String,
    reasons: Vec<String>,
}

impl Oracle {
    const fn not_run(why: String) -> Self {
        Self {
            outcomes: Vec::new(),
            kind: None,
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
    // `oracle-kind scenarios`, `oracle-inbound <topic>`,
    // `oracle-outbound <topic>, <topic>`, `oracle-shared <path>, <path>`,
    // and `oracle-none <why>`.
    fn parse(output: &str) -> Self {
        let mut oracle = Self {
            outcomes: Vec::new(),
            kind: None,
            inbound: None,
            outbound: Vec::new(),
            shared: Vec::new(),
            note: None,
        };
        for line in output.lines().map(str::trim) {
            if let Some(rest) = marked(line, "oracle-kind ") {
                oracle.kind = Some(rest.to_owned());
            } else if let Some(rest) = marked(line, "oracle-inbound ") {
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
        let scenarios = self.kind.as_deref() == Some("scenarios");
        if scenarios {
            writeln!(
                f,
                "- {}/{} scenarios pass: {}.",
                self.passed(),
                self.outcomes.len(),
                groups.join(", ")
            )?;
        } else {
            let inbound = self.inbound.as_deref().unwrap_or("none");
            let outbound = if self.outbound.is_empty() {
                "nothing".to_owned()
            } else {
                spelled(&self.outbound)
            };
            writeln!(
                f,
                "- {}/{} fixtures pass: {}; consumed under `{inbound}`, published to {outbound}.",
                self.passed(),
                self.outcomes.len(),
                groups.join(", ")
            )?;
        }
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
        if scenarios {
            writeln!(f, "\n| scenario | steps | outcome |\n| --- | --- | --- |")?;
        } else {
            writeln!(f, "\n| fixture | recorded | outcome |\n| --- | --- | --- |")?;
        }
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

// The test the eval writes into the built tree as `tests/reference_oracle.rs`,
// `__CRATE__` the root package's crate name: every fixture under `oracle/`
// driven through the guest's messaging router over `omnia_test` doubles,
// its outcome printed in the lines `Oracle::parse` reads.
const HARNESS_SOURCE: &str = r#"//! The eval's reference oracle: every recorded fixture of the reference
//! guest driven through this guest's messaging router. Written by the eval
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
            let router = __CRATE__::router(provider);
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
            let router = __CRATE__::router(provider);
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

// The test the eval writes into the built tree for a reference of recorded
// scenarios, `__CRATE__` the root package's crate name: every scenario of
// `oracle/oracle.json` driven through the guest's HTTP router over
// `omnia_test` doubles and an in-memory table store, its outcome printed in
// the lines `Oracle::parse` reads.
const SCENARIO_HARNESS_SOURCE: &str = r#"//! The eval's reference oracle: every recorded scenario of the source
//! driven through this guest's HTTP router over one fresh store apiece.
//! Written by the eval into the exported tree and removed with it.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::future::{Future, ready};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, SecondsFormat, Utc};
use omnia_sdk::TableStore;
use omnia_sdk::anyhow::{Result as AnyResult, anyhow};
use omnia_sdk::axum::body::Body;
use omnia_sdk::bytes::Bytes;
use omnia_sdk::http::{Method, Request, Response};
use omnia_sdk::sql::{DataType, Field, Row};
use omnia_test::guest::{
    FixedIdentity, MapConfig, MatchedHttp, Memory, MemoryDocs, ScriptedLoader, ScriptedModel,
    ScriptedVcs, Sink, send,
};
use rusqlite::functions::FunctionFlags;
use rusqlite::types::{Value as Sql, ValueRef};
use rusqlite::{Connection, params_from_iter};
use serde_json::{Map, Value};
use tracing::field::{Field as LogField, Visit};
use tracing::{Event, Level, Metadata, Subscriber, span};

// Every upstream the scenarios seed answers under this base; the
// configuration names it as the endpoint of each.
const BASE_URL: &str = "http://api.test";

// The longest a quoted body or event is kept at in a reason.
const QUOTED_CHARS: usize = 240;

// Every capability double the guest reaches, the table store a real
// in-memory database.
#[derive(Clone, Debug, omnia_sdk::Provider)]
struct Oracle {
    #[provider(Config)]
    config: MapConfig,
    #[provider(HttpRequest)]
    http: MatchedHttp,
    #[provider(Identity)]
    identity: FixedIdentity,
    #[provider(Publish)]
    publish: Sink,
    #[provider(Broadcast)]
    broadcast: Sink,
    #[provider(StateStore, BlobStore, Vault)]
    storage: Memory,
    #[provider(DocumentStore)]
    docs: MemoryDocs,
    #[provider(TableStore)]
    tables: Sqlite,
    #[provider(Model)]
    model: ScriptedModel,
    #[provider(Plugins)]
    plugins: ScriptedLoader,
    #[provider(Vcs)]
    vcs: ScriptedVcs,
}

// An in-memory database answering the guest's statements under the
// contract they were written against: `$N` bound by number, each column
// read as the type its declaration names, `expr::type` as a cast. Every
// failed statement is kept with its error, since a guest answering 500 says
// why there alone. `None` is a store that answers nothing, the degraded
// probe's.
#[derive(Clone)]
struct Sqlite {
    conn: Option<Arc<Mutex<Connection>>>,
    errors: Arc<Mutex<Vec<String>>>,
}

impl fmt::Debug for Sqlite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.conn.is_some() { "Sqlite" } else { "Sqlite(unavailable)" })
    }
}

impl Sqlite {
    fn open(schema: &str, tables: &Map<String, Value>) -> Self {
        let conn = Connection::open_in_memory().expect("an in-memory database");
        conn.create_scalar_function("now", 0, FunctionFlags::SQLITE_UTF8, |_| {
            Ok(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true))
        })
        .expect("now()");
        if !schema.trim().is_empty() {
            conn.execute_batch(schema).expect("the schema applies");
        }
        for (table, rows) in tables {
            for row in rows.as_array().into_iter().flatten() {
                let Some(fields) = row.as_object() else { continue };
                let columns: Vec<&str> = fields.keys().map(String::as_str).collect();
                let slots: Vec<String> = (1..=columns.len()).map(|at| format!("?{at}")).collect();
                let sql = format!(
                    "INSERT INTO {table} ({}) VALUES ({})",
                    columns.join(", "),
                    slots.join(", ")
                );
                conn.execute(&sql, params_from_iter(fields.values().map(json_sql)))
                    .unwrap_or_else(|error| panic!("the seed of `{table}` inserts: {error}"));
            }
        }
        Self {
            conn: Some(Arc::new(Mutex::new(conn))),
            errors: Arc::default(),
        }
    }

    fn unavailable() -> Self {
        Self {
            conn: None,
            errors: Arc::default(),
        }
    }

    fn connection(&self) -> AnyResult<MutexGuard<'_, Connection>> {
        self.conn
            .as_ref()
            .map(|conn| conn.lock().expect("connection"))
            .ok_or_else(|| anyhow!("the table store is unavailable"))
    }

    fn failed(&self, sql: &str, error: &rusqlite::Error) -> omnia_sdk::anyhow::Error {
        let text = format!("{error} in `{sql}`");
        self.errors.lock().expect("errors").push(text.clone());
        anyhow!("sql: {text}")
    }

    fn last_error(&self) -> Option<String> {
        self.errors.lock().expect("errors").last().cloned()
    }

    // A statement of the scenario's own, run as written: the rows it changed.
    fn change(&self, sql: &str) -> AnyResult<usize> {
        let conn = self.connection()?;
        conn.execute(sql, []).map_err(|error| anyhow!("{error} in `{sql}`"))
    }

    fn select(&self, query: &str, params: &[DataType]) -> AnyResult<Vec<Row>> {
        let sql = translate(query);
        let conn = self.connection()?;
        let result: rusqlite::Result<Vec<Row>> = (|| {
            let mut statement = conn.prepare(&sql)?;
            let columns: Vec<(String, Option<String>)> = statement
                .columns()
                .map(|column| (column.name().to_owned(), column.decl_type().map(str::to_owned)))
                .collect();
            let mut rows = statement.query(params_from_iter(params.iter().map(to_sql)))?;
            let mut out: Vec<Row> = Vec::new();
            while let Some(row) = rows.next()? {
                let mut fields = Vec::with_capacity(columns.len());
                for (at, (name, declared)) in columns.iter().enumerate() {
                    fields.push(Field {
                        name: name.clone(),
                        value: typed(row.get_ref(at)?, declared.as_deref()),
                    });
                }
                let index = out.len().to_string();
                out.push(Row { index, fields });
            }
            Ok(out)
        })();
        result.map_err(|error| self.failed(&sql, &error))
    }

    fn execute(&self, query: &str, params: &[DataType]) -> AnyResult<u32> {
        let sql = translate(query);
        let conn = self.connection()?;
        conn.execute(&sql, params_from_iter(params.iter().map(to_sql)))
            .map(|changed| u32::try_from(changed).unwrap_or(u32::MAX))
            .map_err(|error| self.failed(&sql, &error))
    }
}

impl TableStore for Sqlite {
    fn query(
        &self, _conn_name: &str, query: &str, params: &[DataType],
    ) -> impl Future<Output = AnyResult<Vec<Row>>> + Send {
        ready(self.select(query, params))
    }

    fn exec(
        &self, _conn_name: &str, query: &str, params: &[DataType],
    ) -> impl Future<Output = AnyResult<u32>> + Send {
        ready(self.execute(query, params))
    }
}

// A statement as the guest wrote it for its production database, in the
// dialect the in-memory database runs.
fn translate(sql: &str) -> String {
    let sql = numbered_params(sql);
    let sql = casts(&sql);
    rewrite_word(&sql, "ILIKE", "LIKE")
}

// `$N` outside a string literal as `?N`, so a parameter binds by its number.
fn numbered_params(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut quoted = false;
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            quoted = !quoted;
        } else if !quoted && c == '$' && chars.peek().is_some_and(char::is_ascii_digit) {
            out.push('?');
            continue;
        }
        out.push(c);
    }
    out
}

// Every `operand::type` outside a string literal as `CAST(operand AS type)`,
// the operand a parenthesised group with the call before it, a literal, or
// a name, parameter, or column path.
fn casts(sql: &str) -> String {
    let mut text = sql.to_owned();
    while let Some(at) = unquoted(&text, "::") {
        let type_end = type_end(&text, at + 2);
        let type_name = text[at + 2..type_end].trim().to_owned();
        let start = operand_start(&text, at);
        if type_name.is_empty() || start == at {
            break;
        }
        let operand = text[start..at].to_owned();
        text.replace_range(start..type_end, &format!("CAST({operand} AS {type_name})"));
    }
    text
}

// The byte at which `needle` first occurs in `sql` outside a string literal.
fn unquoted(sql: &str, needle: &str) -> Option<usize> {
    let mut quoted = false;
    for (at, c) in sql.char_indices() {
        if c == '\'' {
            quoted = !quoted;
        } else if !quoted && sql[at..].starts_with(needle) {
            return Some(at);
        }
    }
    None
}

fn is_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

// Where the type name opening at `from` ends: its words, and a precision
// in parentheses.
fn type_end(sql: &str, from: usize) -> usize {
    let bytes = sql.as_bytes();
    let mut at = from;
    while at < bytes.len() && bytes[at] == b' ' {
        at += 1;
    }
    while at < bytes.len() && is_name(bytes[at]) {
        at += 1;
    }
    if at < bytes.len() && bytes[at] == b'(' {
        while at < bytes.len() && bytes[at] != b')' {
            at += 1;
        }
        at = (at + 1).min(bytes.len());
    }
    at
}

// Where the operand ending at `end` starts.
fn operand_start(sql: &str, end: usize) -> usize {
    let bytes = sql.as_bytes();
    let mut at = end;
    if at > 0 && bytes[at - 1] == b')' {
        let mut depth = 0usize;
        while at > 0 {
            at -= 1;
            match bytes[at] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        while at > 0 && is_name(bytes[at - 1]) {
            at -= 1;
        }
        return at;
    }
    if at > 0 && bytes[at - 1] == b'\'' {
        at -= 1;
        while at > 0 {
            at -= 1;
            if bytes[at] == b'\'' {
                break;
            }
        }
        return at;
    }
    while at > 0 && (is_name(bytes[at - 1]) || matches!(bytes[at - 1], b'.' | b'?' | b'$')) {
        at -= 1;
    }
    at
}

// Every whole word `from`, in any case, outside a string literal, as `to`.
fn rewrite_word(sql: &str, from: &str, to: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut quoted = false;
    let mut at = 0;
    while at < sql.len() {
        let c = bytes[at];
        if c == b'\'' {
            quoted = !quoted;
        }
        let bounded = at + from.len() <= sql.len()
            && sql[at..at + from.len()].eq_ignore_ascii_case(from)
            && (at == 0 || !is_name(bytes[at - 1]))
            && sql.as_bytes().get(at + from.len()).is_none_or(|next| !is_name(*next));
        if !quoted && bounded {
            out.push_str(to);
            at += from.len();
            continue;
        }
        out.push(c as char);
        at += 1;
    }
    out
}

fn to_sql(value: &DataType) -> Sql {
    match value {
        DataType::Boolean(Some(b)) => Sql::Integer(i64::from(*b)),
        DataType::Int32(Some(i)) => Sql::Integer(i64::from(*i)),
        DataType::Int64(Some(i)) => Sql::Integer(*i),
        DataType::Uint32(Some(u)) => Sql::Integer(i64::from(*u)),
        DataType::Uint64(Some(u)) => Sql::Integer(i64::try_from(*u).unwrap_or(i64::MAX)),
        DataType::Float(Some(f)) => Sql::Real(f64::from(*f)),
        DataType::Double(Some(f)) => Sql::Real(*f),
        DataType::Str(Some(s))
        | DataType::Date(Some(s))
        | DataType::Time(Some(s))
        | DataType::Timestamp(Some(s)) => Sql::Text(s.clone()),
        DataType::Binary(Some(b)) => Sql::Blob(b.clone()),
        _ => Sql::Null,
    }
}

fn json_sql(value: &Value) -> Sql {
    match value {
        Value::Null => Sql::Null,
        Value::Bool(b) => Sql::Integer(i64::from(*b)),
        Value::Number(n) => n.as_i64().map_or_else(
            || Sql::Real(n.as_f64().unwrap_or_default()),
            Sql::Integer,
        ),
        Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}

// A column's value as the type its declaration names; an expression,
// declared nothing, as the value is stored.
fn typed(value: ValueRef<'_>, declared: Option<&str>) -> DataType {
    let declared = declared.map(str::to_ascii_uppercase).unwrap_or_default();
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    if declared.contains("BOOL") {
        return DataType::Boolean(match value {
            ValueRef::Integer(i) => Some(i != 0),
            ValueRef::Real(f) => Some(f != 0.0),
            ValueRef::Text(t) => Some(matches!(
                text(t).to_ascii_lowercase().as_str(),
                "1" | "t" | "true" | "y" | "yes"
            )),
            _ => None,
        });
    }
    if declared.contains("INT") {
        return DataType::Int64(match value {
            ValueRef::Integer(i) => Some(i),
            ValueRef::Real(f) => Some(f as i64),
            ValueRef::Text(t) => text(t).parse().ok(),
            _ => None,
        });
    }
    if ["DOUBLE", "REAL", "FLOAT", "NUMERIC", "DECIMAL"].iter().any(|kind| declared.contains(kind)) {
        return DataType::Double(match value {
            ValueRef::Integer(i) => Some(i as f64),
            ValueRef::Real(f) => Some(f),
            ValueRef::Text(t) => text(t).parse().ok(),
            _ => None,
        });
    }
    if declared.contains("TIMESTAMP") {
        return DataType::Timestamp(stored_text(value));
    }
    if declared.contains("DATE") {
        return DataType::Date(stored_text(value));
    }
    if declared.contains("TIME") {
        return DataType::Time(stored_text(value));
    }
    if !declared.is_empty() {
        return DataType::Str(stored_text(value));
    }
    match value {
        ValueRef::Null => DataType::Str(None),
        ValueRef::Integer(i) => DataType::Int64(Some(i)),
        ValueRef::Real(f) => DataType::Double(Some(f)),
        ValueRef::Text(t) => DataType::Str(Some(text(t))),
        ValueRef::Blob(b) => DataType::Binary(Some(b.to_vec())),
    }
}

fn stored_text(value: ValueRef<'_>) -> Option<String> {
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(f) => Some(f.to_string()),
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
    }
}

// The guest's warn and error events as one scenario runs, since a handler
// that logs a failure and answers an error says why here alone.
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
    fn record_debug(&mut self, field: &LogField, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.0.insert_str(0, &format!("{value:?} "));
        } else {
            let _ = write!(self.0, "{}={value:?} ", field.name());
        }
    }
}

// A text as a reason reads it: one line, within the quote length, with the
// separators the oracle's lines are split on spelled otherwise.
fn quoted(line: &str) -> String {
    shortened(&flat(line), QUOTED_CHARS)
}

fn flat(text: &str) -> String {
    text.replace('\n', " ").replace(" | ", " / ").replace("; ", ", ")
}

fn shortened(text: &str, chars: usize) -> String {
    let mut short: String = text.chars().take(chars).collect();
    if text.chars().count() > chars {
        short.push('…');
    }
    short
}

fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle")
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

// What every scenario runs over: the configuration, the bearer the guarded
// paths take, the store's schema and seeds, and each upstream path's
// default answer.
struct Settings {
    config: Vec<(String, String)>,
    header: String,
    token: String,
    guarded: String,
    schema: String,
    tables: Map<String, Value>,
    http: Map<String, Value>,
}

impl Settings {
    fn read(oracle: &Value) -> Self {
        Self {
            config: oracle["config"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(key, value)| (key.clone(), value.as_str().unwrap_or("").to_owned()))
                .collect(),
            header: oracle["auth"]["header"].as_str().unwrap_or("authorization").to_owned(),
            token: oracle["auth"]["token"].as_str().unwrap_or("").to_owned(),
            guarded: oracle["auth"]["guarded"].as_str().unwrap_or("/").to_owned(),
            schema: oracle["schema"].as_str().unwrap_or("").to_owned(),
            tables: oracle["tables"].as_object().cloned().unwrap_or_default(),
            http: oracle["http"].as_object().cloned().unwrap_or_default(),
        }
    }

    // The provider one scenario runs over: a fresh store seeded, the
    // upstream answers the scenario seeds, and the configuration.
    fn provider(&self, seed: &Value) -> Oracle {
        let tables = if seed["tables"].as_str() == Some("unavailable") {
            Sqlite::unavailable()
        } else {
            Sqlite::open(&self.schema, &self.tables)
        };
        Oracle {
            config: MapConfig::default().with(self.config.iter().cloned()),
            http: self.upstream(&seed["http"]),
            identity: FixedIdentity::new(&self.token),
            publish: Sink::default(),
            broadcast: Sink::default(),
            storage: Memory::default(),
            docs: MemoryDocs::default(),
            tables,
            model: ScriptedModel::default(),
            plugins: ScriptedLoader::default(),
            vcs: ScriptedVcs::default(),
        }
    }

    // Every upstream answer the scenario seeds, at its path and query in the
    // order given and, for two parameters, the other; a body the entry
    // leaves out is the path's default, a status of 4xx or 5xx an error.
    fn upstream(&self, entries: &Value) -> MatchedHttp {
        let mut http = MatchedHttp::default();
        for entry in entries.as_array().into_iter().flatten() {
            let method = entry["method"].as_str().unwrap_or("GET");
            let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
            let path = entry["path"].as_str().unwrap_or("/");
            let status = u16::try_from(entry["status"].as_u64().unwrap_or(200)).unwrap_or(500);
            let body = match entry.get("body") {
                Some(body) => body.to_string(),
                None if status >= 400 => Value::Null.to_string(),
                None => self.http.get(path).cloned().unwrap_or(Value::Null).to_string(),
            };
            let params: Vec<(String, String)> = entry["query"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(key, value)| {
                    let value = match value {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    };
                    (key.clone(), value)
                })
                .collect();
            let mut orders = vec![params.clone()];
            if params.len() == 2 {
                orders.push(vec![params[1].clone(), params[0].clone()]);
            }
            for order in orders {
                let query: Vec<String> =
                    order.iter().map(|(key, value)| format!("{key}={value}")).collect();
                let url = if query.is_empty() {
                    format!("{BASE_URL}{path}")
                } else {
                    format!("{BASE_URL}{path}?{}", query.join("&"))
                };
                let response = Response::builder()
                    .status(status)
                    .header("content-type", "application/json")
                    .body(Bytes::from(body.clone()))
                    .expect("upstream response");
                http = http.on(method.clone(), url, response);
            }
        }
        http
    }
}

#[tokio::test]
async fn reference_scenarios() {
    tokio::task::LocalSet::new().run_until(run()).await;
}

async fn run() {
    let dir = oracle_dir();
    let oracle: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("oracle.json")).expect("oracle.json"))
            .expect("oracle.json parses");
    let settings = Settings::read(&oracle);
    println!("oracle-kind scenarios");
    for scenario in oracle["scenarios"].as_array().into_iter().flatten() {
        let name = scenario["name"].as_str().unwrap_or("?");
        let driven = drive(&settings, scenario).await;
        if driven.reasons.is_empty() {
            println!("oracle {name} | {} | pass", driven.expected);
        } else {
            println!("oracle {name} | {} | fail: {}", driven.expected, driven.reasons.join("; "));
        }
    }
}

// What one scenario driven through the guest showed.
struct Driven {
    expected: String,
    reasons: Vec<String>,
}

// One scenario through the guest, step by step over one provider: the first
// step whose answer differs ends it, with the last failed statement and the
// last event the guest logged beside the difference.
async fn drive(settings: &Settings, scenario: &Value) -> Driven {
    let steps: Vec<&Value> = scenario["steps"].as_array().into_iter().flatten().collect();
    let last_status = steps
        .iter()
        .rev()
        .find_map(|step| step["expect"]["status"].as_u64())
        .unwrap_or_default();
    let expected = match steps.len() {
        1 => format!("1 step to {last_status}"),
        count => format!("{count} steps to {last_status}"),
    };
    let provider = settings.provider(&scenario["seed"]);
    let tables = provider.tables.clone();
    let logged = Logged::default();
    let router = __CRATE__::router(provider);
    let mut captures: BTreeMap<String, String> = BTreeMap::new();
    let mut reasons = Vec::new();

    for (index, step) in steps.iter().enumerate() {
        let label = format!("step {} of {}", index + 1, steps.len());

        // a statement of the scenario's own, over the store behind the guest
        if let Some(sql) = step["sql"].as_str() {
            let sql = substitute(sql, &captures);
            match tables.change(&sql) {
                Ok(0) => reasons.push(format!("{label}: `{sql}` changed no row")),
                Ok(_) => {}
                Err(error) => reasons.push(format!("{label}: {}", quoted(&error.to_string()))),
            }
            if reasons.is_empty() {
                continue;
            }
            break;
        }

        // the request through the router
        let request = match build_request(&step["request"], settings, &captures) {
            Ok(request) => request,
            Err(why) => {
                reasons.push(format!("{label}: {why}"));
                break;
            }
        };
        let outcome = {
            let _default = tracing::subscriber::set_default(logged.clone());
            let router = router.clone();
            tokio::task::spawn_local(async move { send(&router, request).await }).await
        };
        let response = match outcome {
            Ok(response) => response,
            Err(join) => {
                let text = panic_text(join);
                let reason = match text.strip_prefix("no response scripted for ") {
                    Some(fetched) => format!("fetched {fetched}, which the scenario does not seed"),
                    None => format!("panicked: {text}"),
                };
                reasons.push(format!("{label}: {}", quoted(&reason)));
                break;
            }
        };
        let status = response.status().as_u16();
        let text = String::from_utf8_lossy(response.body()).into_owned();
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);

        // the status first, since an error body answers nothing else asked
        let expect = &step["expect"];
        let wanted = expect["status"].as_u64().and_then(|status| u16::try_from(status).ok());
        if let Some(wanted) = wanted.filter(|wanted| *wanted != status) {
            reasons.push(format!(
                "{label}: status {status}, {wanted} expected (answered {})",
                quoted(text.trim())
            ));
            break;
        }
        if !expect["body"].is_null() && body.is_null() && !text.trim().is_empty() {
            reasons.push(format!("{label}: the body is no JSON (answered {})", quoted(text.trim())));
            break;
        }

        // the captures, before the expectations name them
        for (name, path) in step["capture"].as_object().into_iter().flatten() {
            let path = path.as_str().unwrap_or("");
            match value_at(&body, path) {
                Some(Value::String(value)) => {
                    captures.insert(name.clone(), value.clone());
                }
                Some(other) => {
                    captures.insert(name.clone(), other.to_string());
                }
                None => reasons.push(format!("{label}: `{path}` is absent from the body")),
            }
        }

        // the headers and the body
        let mut differences = Vec::new();
        for (header, want) in expect["headers"].as_object().into_iter().flatten() {
            let want = substitute(want.as_str().unwrap_or(""), &captures);
            let have = response.headers().get(header.as_str()).and_then(|value| value.to_str().ok());
            if have != Some(want.as_str()) {
                differences.push(format!(
                    "header `{header}` is {}, `{want}` expected",
                    have.map_or_else(|| "absent".to_owned(), |have| format!("`{}`", flat(have)))
                ));
            }
        }
        if !expect["body"].is_null() {
            compare(&expect["body"], &body, "body", &captures, &mut differences);
        }
        if !differences.is_empty() {
            reasons.push(format!("{label}: {}", differences.join(", ")));
        }
        if !reasons.is_empty() {
            break;
        }
    }

    // what the store and the guest said as it failed
    if !reasons.is_empty() {
        if let Some(error) = tables.last_error() {
            reasons.push(format!("sql: {}", quoted(&error)));
        }
        if let Some(last) = logged.last() {
            reasons.push(format!("logged: {}", quoted(&last)));
        }
    }
    Driven { expected, reasons }
}

// The step's request: its method, path, headers, and JSON body with every
// capture substituted, and the bearer on a guarded path the step leaves to
// the harness.
fn build_request(
    spec: &Value, settings: &Settings, captures: &BTreeMap<String, String>,
) -> Result<Request<Body>, String> {
    let method = spec["method"].as_str().unwrap_or("GET");
    let path = substitute(spec["path"].as_str().unwrap_or("/"), captures);
    let mut builder = Request::builder().method(method).uri(path.as_str());
    let mut authorised = false;
    for (name, value) in spec["headers"].as_object().into_iter().flatten() {
        if name.eq_ignore_ascii_case(&settings.header) {
            authorised = true;
        }
        if let Some(text) = value.as_str() {
            builder = builder.header(name.as_str(), substitute(text, captures));
        }
    }
    if path.starts_with(&settings.guarded) && !authorised {
        builder = builder.header(settings.header.as_str(), format!("Bearer {}", settings.token));
    }
    let body = &spec["body"];
    let request = if body.is_null() {
        builder.body(Body::empty())
    } else {
        builder
            .header("content-type", "application/json")
            .body(Body::from(substitute_value(body, captures).to_string()))
    };
    request.map_err(|error| format!("the request is malformed: {error}"))
}

// `text` with every `{name}` a capture holds spelled as its value.
fn substitute(text: &str, captures: &BTreeMap<String, String>) -> String {
    let mut out = text.to_owned();
    for (name, value) in captures {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

fn substitute_value(value: &Value, captures: &BTreeMap<String, String>) -> Value {
    match value {
        Value::String(text) => Value::String(substitute(text, captures)),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| substitute_value(item, captures)).collect())
        }
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, field)| (key.clone(), substitute_value(field, captures)))
                .collect(),
        ),
        other => other.clone(),
    }
}

// The value at a dotted `path` of `body`, a numeric segment an index.
fn value_at<'a>(body: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = body;
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        current = match segment.parse::<usize>() {
            Ok(index) => current.get(index)?,
            Err(_) => current.get(segment)?,
        };
    }
    Some(current)
}

// Every way `have` differs from `want`: an object's listed fields each
// present and agreeing, an array of the same length agreeing item by item,
// a string equal once captures are spelled, or holding to its placeholder,
// a number equal in value, a boolean or null equal.
fn compare(
    want: &Value, have: &Value, path: &str, captures: &BTreeMap<String, String>,
    differences: &mut Vec<String>,
) {
    match want {
        Value::Object(fields) => {
            let Some(object) = have.as_object() else {
                differences.push(format!("`{path}` is {}, an object expected", brief(have)));
                return;
            };
            for (key, want) in fields {
                match object.get(key) {
                    Some(have) => compare(want, have, &format!("{path}.{key}"), captures, differences),
                    None => differences.push(format!("`{path}.{key}` is absent")),
                }
            }
        }
        Value::Array(items) => {
            let Some(array) = have.as_array() else {
                differences.push(format!("`{path}` is {}, an array expected", brief(have)));
                return;
            };
            if array.len() != items.len() {
                differences.push(format!(
                    "`{path}` holds {} items, {} expected",
                    array.len(),
                    items.len()
                ));
                return;
            }
            for (index, (want, have)) in items.iter().zip(array).enumerate() {
                compare(want, have, &format!("{path}[{index}]"), captures, differences);
            }
        }
        Value::String(text) => {
            let agrees = match text.as_str() {
                "{uuid}" => have.as_str().is_some_and(is_uuid),
                "{timestamp}" => {
                    have.as_str().is_some_and(|text| DateTime::parse_from_rfc3339(text).is_ok())
                }
                "{any}" => !have.is_null(),
                _ => have.as_str() == Some(substitute(text, captures).as_str()),
            };
            if !agrees {
                differences.push(format!(
                    "`{path}` is {}, `{}` expected",
                    brief(have),
                    substitute(text, captures)
                ));
            }
        }
        Value::Number(number) => {
            let agrees = match (have.as_f64(), number.as_f64()) {
                (Some(have), Some(want)) => (have - want).abs() < 1e-9,
                _ => false,
            };
            if !agrees {
                differences.push(format!("`{path}` is {}, {number} expected", brief(have)));
            }
        }
        Value::Bool(_) | Value::Null => {
            if want != have {
                differences.push(format!("`{path}` is {}, {want} expected", brief(have)));
            }
        }
    }
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(at, c)| match at {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

// A value as a reason quotes it: `absent` for null, else its JSON within
// a short length.
fn brief(value: &Value) -> String {
    if value.is_null() {
        return "absent".to_owned();
    }
    format!("`{}`", shortened(&flat(&value.to_string()), 80))
}

// The dialect translation held to the statements a port writes.
#[test]
fn translations() {
    assert_eq!(
        translate("SELECT count(*)::text AS total FROM orders WHERE state = $1 AND customer_id = $2"),
        "SELECT CAST(count(*) AS text) AS total FROM orders WHERE state = ?1 AND customer_id = ?2"
    );
    assert_eq!(translate("UPDATE customers SET tier = $2 WHERE id = $1"), "UPDATE customers SET tier = ?2 WHERE id = ?1");
    assert_eq!(translate("SELECT $1::uuid, 'a::b'"), "SELECT CAST(?1 AS uuid), 'a::b'");
    assert_eq!(translate("SELECT (a + b)::int, x::varchar(10)"), "SELECT CAST((a + b) AS int), CAST(x AS varchar(10))");
    assert_eq!(translate("SELECT 1 FROM c WHERE email ILIKE $1 AND note = 'ILIKE'"), "SELECT 1 FROM c WHERE email LIKE ?1 AND note = 'ILIKE'");
    assert_eq!(translate("SELECT id FROM orders LIMIT $3 OFFSET $4"), "SELECT id FROM orders LIMIT ?3 OFFSET ?4");
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_output() {
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

        let output = "\
        oracle-inbound dev-orders.v1
        oracle static/0001 | 2 events | fail: published 0 events, 2 recorded; logged: WARN orders::inbound: rates missing route=12
        oracle replay/4247 | 0 events | pass
        oracle-outbound dev-order-positions.v1
        oracle-shared /stops, /routes?all=true
        ";
        let oracle = Oracle::parse(output);
        assert_eq!(oracle.shared, ["/stops", "/routes?all=true"]);
        assert_eq!(
            oracle.outcomes[0].reasons[1],
            "logged: WARN orders::inbound: rates missing route=12"
        );
        let text = oracle.to_string();
        assert!(
            text.contains("- `/stops`, `/routes?all=true` answered from another fixture's"),
            "{text}"
        );
        assert_eq!(Oracle::parse("oracle-outbound \n").shared, Vec::<String>::new());

        let oracle = Oracle::parse("oracle-none no inbound topic consumed among: dev-a, a\n");
        assert_eq!(oracle.cell(), "not run");
        assert!(
            oracle.to_string().contains("- not run: no inbound topic consumed among: dev-a, a")
        );

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

        let output = "\
        oracle-kind scenarios
        oracle orders/place-standard | 2 steps to 201 | pass
        oracle orders/cancel-shipped | 4 steps to 409 | fail: step 3 of 4: status 200, 409 expected (answered {}); sql: no such column: foo in `SELECT foo`
        oracle start/missing-bearer | 1 steps to 401 | fail: step 1 of 1: status 201, 401 expected
        ";
        let oracle = Oracle::parse(output);
        assert_eq!(oracle.kind.as_deref(), Some("scenarios"));
        assert_eq!(oracle.cell(), "1/3");
        assert_eq!(oracle.groups(), [("orders".to_owned(), 1, 2), ("start".to_owned(), 0, 1)]);
        assert_eq!(oracle.outcomes[1].reasons.len(), 2);
        let text = oracle.to_string();
        assert!(text.contains("- 1/3 scenarios pass: 1/2 orders, 0/1 start."), "{text}");
        assert!(text.contains("| scenario | steps | outcome |"), "{text}");
        assert!(!text.contains("consumed under"), "{text}");
    }

    #[test]
    fn config_and_topics() {
        let text = "pub const ENV: &str = \"ENV\";\n/// doc\npub const URL: &str = \"BLOCK_MGT_URL\";\npub const N: u32 = 3;\n";
        let consts = str_consts(text);
        assert_eq!(consts.into_iter().collect::<Vec<_>>(), ["BLOCK_MGT_URL", "ENV"]);

        let text = "// Where the upstream answers.\npub(crate) const RATES_URL: &str = \"RATES_URL\";\npub(crate) const ORDERS: &str = \"/orders\";\nconst TTL: u64 = 600;\npub const KEY: &str = \"orders/rates\";\n";
        let consts = setting_consts(text);
        assert_eq!(consts.into_iter().collect::<Vec<_>>(), ["RATES_URL"]);

        let text = "// The settings the part reads: `ORDERS_RATE_URL`, `ORDERS_TYPE`.\n\
                            #[derive(Debug, Settings)]\n#[setting(prefix = \"ORDERS_\")]\n\
                            pub(crate) struct PlacingSettings {\n    // Where the rates are: the upstream.\n    \
                            rate_url: String,\n    #[setting(key = \"ORDERS_TYPE\")]\n    type_: String,\n    \
                            ttl_secs: u64,\n}\n\n#[derive(Debug, Deserialize)]\npub struct Order {\n    \
                            pub id: String,\n}\n\n#[derive(Settings)]\nstruct Probe {\n    pub(crate) \
                            deadline_ms: u64,\n}\n";
        let keys = setting_fields(text);
        assert_eq!(
            keys.into_iter().collect::<Vec<_>>(),
            ["DEADLINE_MS", "ORDERS_RATE_URL", "ORDERS_TTL_SECS", "ORDERS_TYPE"]
        );

        let text = "pub mod http {}\n\npub mod topic {\n    pub const TRAIN: &str = \"realtime-train.v1\";\n    pub mod inner { pub const X: &str = \"x\"; }\n}\npub const OUT: &str = \"outside\";\n";
        let topics = str_consts(&module_block(text, "topic"));
        assert_eq!(topics.into_iter().collect::<Vec<_>>(), ["realtime-train.v1", "x"]);
        assert_eq!(module_block(text, "absent"), "");

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

        let table: BTreeMap<String, String> = [
            ("GST_RATE", "0.15"),
            ("SHIPPING_API_URL", "http://api.test"),
            ("API_TOKENS", "test-token"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
        let keys: BTreeSet<String> = [
            "ORDERS_GST_RATE",
            "RATES_SHIPPING_API_URL",
            "START_API_TOKENS",
            "ORDERS_CACHE_TTL_SECONDS",
            "HEALTH_DEADLINE_MS",
            "START_BEARER_TOKEN",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let seed = scenario_config(&table, "test-token", &keys);
        assert_eq!(seed["GST_RATE"], "0.15");
        assert_eq!(seed["ORDERS_GST_RATE"], "0.15");
        assert_eq!(seed["RATES_SHIPPING_API_URL"], "http://api.test");
        assert_eq!(seed["START_API_TOKENS"], "test-token");
        assert_eq!(seed["ORDERS_CACHE_TTL_SECONDS"], "60");
        assert_eq!(seed["START_BEARER_TOKEN"], "test-token");
        assert!(!seed.contains_key("HEALTH_DEADLINE_MS"));
    }

    // A prefixed struct's fields read under its prefix, a field's own key
    // over it, an unprefixed struct's bare; a struct deriving no `Settings`
    // and a field comment spelling a colon add nothing.

    #[test]
    fn dev_dependencies() {
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
        let additions = dev_dependency_additions(&manifest, &[]).unwrap();
        assert!(additions.contains(".dev-dependencies.chrono]\nworkspace = true"), "{additions}");
        assert!(
            additions.contains(".dev-dependencies.chrono-tz]\nversion = \"0.10\""),
            "{additions}"
        );
        assert!(additions.contains(".dev-dependencies.tracing]\nversion = \"0.1\""), "{additions}");
        assert!(!additions.contains("serde_json"), "{additions}");
        assert!(!additions.contains("rusqlite"), "{additions}");
        let appended: toml::Table =
            format!("{}\n{additions}", toml::to_string(&manifest).unwrap()).parse().unwrap();
        assert_eq!(package_dependency_names(&appended).len(), 8);
        assert!(dev_dependency_additions(&appended, &[]).unwrap().is_empty(), "added once");

        // the scenario harness adds its database with the features it reads
        let scenario = dev_dependency_additions(&manifest, &SCENARIO_DEPENDENCIES).unwrap();
        assert!(
            scenario.contains(
                ".dev-dependencies.rusqlite]\nversion = \"0.41\"\nfeatures = [\"bundled\", \
                 \"column_decltype\", \"functions\"]\n"
            ),
            "{scenario}"
        );

        // a crate the package depends on already serves its tests
        let depending: toml::Table = "\
[package]
name = \"guest\"
[dependencies]
tracing = \"0.1\"
[dev-dependencies]
omnia-test = \"0.36\"
tokio = \"1\"
"
        .parse()
        .unwrap();
        assert!(!dev_dependency_additions(&depending, &[]).unwrap().contains("tracing"));

        let bare: toml::Table = "[package]\nname = \"guest\"\n".parse().unwrap();
        assert_eq!(
            dev_dependency_additions(&bare, &[]).unwrap_err(),
            "the built tree's root package has no `omnia-test` dev-dependency"
        );
    }

    #[test]
    fn oracle_spec() {
        let spec = Spec::parse(
            "kind = \"http\"\n[auth]\nheader = \"authorization\"\ntoken = \"t\"\nguarded = \
             \"/api\"\n[config]\nGST_RATE = \"0.15\"\n",
        )
        .unwrap();
        assert_eq!(spec.auth.guarded, "/api");
        assert_eq!(spec.config["GST_RATE"], "0.15");
        let kind = Spec::parse(
            "kind = \"grpc\"\n[auth]\nheader = \"a\"\ntoken = \"t\"\nguarded = \"/\"\n",
        )
        .unwrap_err();
        assert!(kind.contains("`http` is the one kind"), "{kind}");
        let guarded = Spec::parse(
            "kind = \"http\"\n[auth]\nheader = \"a\"\ntoken = \"t\"\nguarded = \"api\"\n",
        )
        .unwrap_err();
        assert!(guarded.contains("no path"), "{guarded}");
        Spec::parse("kind = \"http\"\nextra = 1\n").unwrap_err();
    }

    #[test]
    fn scenario_schema() {
        let sound = serde_json::json!({
            "seed": { "http": [{ "path": "/rates", "query": { "zone": "z" } }] },
            "steps": [
                {
                    "request": { "method": "POST", "path": "/api/orders", "body": { "n": 1 } },
                    "capture": { "order": "id" },
                    "expect": { "status": 201, "headers": { "location": "/api/orders/{order}" },
                                "body": { "id": "{uuid}", "createdAt": "{timestamp}" } }
                },
                { "sql": "UPDATE orders SET state = 'paid' WHERE id = '{order}'" },
                { "request": { "method": "GET", "path": "/api/orders/{order}" },
                  "expect": { "status": 200, "body": { "total": "{any}" } } }
            ]
        });
        check_scenario(&sound).unwrap();

        let unknown = serde_json::json!({
            "steps": [{ "request": { "method": "GET", "path": "/api/orders/{order}" },
                        "expect": { "status": 200 } }]
        });
        assert_eq!(
            check_scenario(&unknown).unwrap_err(),
            "step 1 spells `{order}`, which no earlier step captures"
        );
        let both = serde_json::json!({
            "steps": [{ "request": { "method": "GET", "path": "/" }, "sql": "SELECT 1",
                        "expect": { "status": 200 } }]
        });
        assert!(check_scenario(&both).unwrap_err().contains("not both or neither"));
        let status = serde_json::json!({
            "steps": [{ "request": { "method": "GET", "path": "/" }, "expect": {} }]
        });
        assert_eq!(check_scenario(&status).unwrap_err(), "step 1 has no `expect.status`");
        let empty = serde_json::json!({ "steps": [] });
        assert_eq!(check_scenario(&empty).unwrap_err(), "has no `steps`");
        let seed = serde_json::json!({
            "seed": { "http": [{ "query": {} }] },
            "steps": [{ "request": { "method": "GET", "path": "/" }, "expect": { "status": 200 } }]
        });
        assert!(check_scenario(&seed).unwrap_err().contains("no `path`"));
    }

    // The table's own keys stand; a built key the table lacks takes the
    // value of its longest suffix in the table, else the pattern seed, and
    // is left unset where neither reads it.

    #[test]
    fn comparison_summary() {
        let comparison = Comparison {
            name: "evals/cases/express-orders/oracle".to_owned(),
            measures: Vec::new(),
            oracle: Oracle::parse(
                "oracle-kind scenarios\noracle orders/place-standard | 2 steps to 201 | pass\n",
            ),
            log: String::new(),
        };
        assert_eq!(comparison.finding(), "reference oracle 1/1");
        let text = comparison.to_string();
        assert!(text.contains("recorded as scenarios"), "{text}");
        assert!(!text.contains("| measure |"), "{text}");

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
            log: String::new(),
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

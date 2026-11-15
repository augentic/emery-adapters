//! Reads an `evolution.toml` chain and resolves its step overlays against a base case.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};
use std::{fs, io};

use serde::Deserialize;
use serde_json::Value;

use crate::case::{CASES, Case, check_tree};
use crate::fidelity::Target;
use crate::reference::Reference;
use crate::stage::{copy_tree, skipped_entry};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvolution {
    base: String,
    target: String,
    chains: NonZeroUsize,
    steps: Vec<String>,
}

pub struct Evolution {
    name: String,
    pub base: Case,
    fixture_dir: PathBuf,
    oracle_dir: PathBuf,
    pub target: Target,
    pub chains: NonZeroUsize,
    pub steps: Vec<Step>,
}

impl Evolution {
    pub const FILE: &str = "evolution.toml";

    fn read(root: &Path, name: &str) -> Result<Self, String> {
        let dir = root.join(CASES).join(name);
        let text = fs::read_to_string(dir.join(Self::FILE))
            .map_err(|error| format!("{name}: {}: {error}", Self::FILE))?;
        let raw: RawEvolution =
            toml::from_str(&text).map_err(|error| format!("{name}: {}: {error}", Self::FILE))?;
        if raw.steps.is_empty() {
            return Err(format!("{name}: {} lists no `steps`", Self::FILE));
        }
        let target = Target::parse(&raw.target)
            .map_err(|error| format!("{name}: {}: {error}", Self::FILE))?;
        let base = Case::read(root, &raw.base)
            .map_err(|error| format!("{name}: base `{}`: {error}", raw.base))?;
        let [source] = base.expected.sources.as_slice() else {
            return Err(format!(
                "{name}: base `{}` must declare one workspace `fixture` source",
                raw.base
            ));
        };
        let Some(fixture_rel) = source.fixture.as_deref() else {
            return Err(format!(
                "{name}: base `{}` must declare one workspace `fixture` source",
                raw.base
            ));
        };
        let Some(oracle_rel) = base.expected.reference.as_deref() else {
            return Err(format!("{name}: base `{}` declares no scenario `reference`", raw.base));
        };
        let fixture_dir = root.join(fixture_rel);
        let oracle_dir = root.join(oracle_rel);
        check_oracle_tree(&oracle_dir, oracle_rel)
            .map_err(|error| format!("{name}: base oracle `{oracle_rel}`: {error}"))?;

        let mut seen = BTreeSet::new();
        let mut steps = Vec::with_capacity(raw.steps.len());
        for (index, slug) in raw.steps.iter().enumerate() {
            let number = index + 1;
            let prefix = format!("{number:02}-");
            if !slug.starts_with(&prefix) || slug.len() == prefix.len() {
                return Err(format!(
                    "{name}: step {number} `{slug}` must start with `{prefix}<name>`"
                ));
            }
            if !seen.insert(slug.as_str()) {
                return Err(format!("{name}: step `{slug}` is listed twice"));
            }
            let step = Step::read(&dir.join("steps").join(slug), number, slug)
                .map_err(|error| format!("{name}: steps/{slug}: {error}"))?;
            steps.push(step);
        }

        Ok(Self {
            name: name.to_owned(),
            base,
            fixture_dir,
            oracle_dir,
            target,
            chains: raw.chains,
            steps,
        })
    }

    // Copies the base fixture and oracle into `scratch` and applies every step
    // overlay and removal in sequence, holding each intermediate tree and
    // scenario set to the credential and scenario gates.
    pub fn dry_run(&self, scratch: &Path) -> Result<ResolvedChain, String> {
        if scratch.exists() {
            fs::remove_dir_all(scratch)
                .map_err(|error| format!("{}: {error}", scratch.display()))?;
        }
        let (source, baseline_source_files) =
            StagedSource::seed(&self.fixture_dir, scratch.join("source"))?;
        let (mut oracle, baseline_scenarios) =
            StagedOracle::seed(&self.oracle_dir, scratch.join("oracle"))?;

        let mut steps = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            steps.push(step.resolve(&source, &mut oracle)?);
        }

        Ok(ResolvedChain {
            name: self.name.clone(),
            base: self.base.name.clone(),
            target: self.target,
            chains: self.chains.get(),
            baseline_source_files,
            baseline_scenarios,
            steps,
        })
    }
}

pub struct Step {
    number: usize,
    slug: String,
    dir: PathBuf,
    spec: StepToml,
    patch_files: Vec<String>,
    oracle_files: Vec<String>,
    oracle_scenarios: Vec<String>,
}

impl Step {
    const FILE: &str = "step.toml";

    fn read(dir: &Path, number: usize, slug: &str) -> Result<Self, String> {
        let text = fs::read_to_string(dir.join(Self::FILE))
            .map_err(|error| format!("{}: {error}", Self::FILE))?;
        let spec = StepToml::parse(&text).map_err(|error| format!("{} {error}", Self::FILE))?;

        // source overlay files
        let patch_dir = dir.join("patch");
        let patch_files = if patch_dir.is_dir() {
            if matches!(spec.input, StepInput::Intent { .. }) {
                return Err("`via = \"intent\"` cannot carry a `patch/` directory".to_owned());
            }
            check_tree(&patch_dir, &format!("{slug}/patch"))?;
            list_files(&patch_dir).map_err(|error| format!("patch/: {error}"))?
        } else {
            Vec::new()
        };
        if let StepInput::Patch { remove } = &spec.input
            && patch_files.is_empty()
            && remove.is_empty()
        {
            return Err(
                "`via = \"patch\"` must overlay at least one file under `patch/` or list `remove`"
                    .to_owned(),
            );
        }

        // oracle overlay files and scenarios
        let oracle_dir = dir.join("oracle");
        let mut oracle_files = Vec::new();
        let mut oracle_scenarios = Vec::new();
        if oracle_dir.is_dir() {
            check_tree(&oracle_dir, &format!("{slug}/oracle"))?;
            for rel in list_files(&oracle_dir).map_err(|error| format!("oracle/: {error}"))? {
                if let Some(scenario) = rel.strip_prefix("scenarios/") {
                    check_scenario_path(scenario, "oracle/scenarios")?;
                    check_scenario_file(&oracle_dir.join(&rel), &format!("oracle/{rel}"))?;
                    oracle_scenarios.push(scenario.to_owned());
                } else if rel == Reference::SPEC_FILE {
                    check_oracle_spec_file(&oracle_dir.join(&rel), &format!("oracle/{rel}"))?;
                    oracle_files.push(rel);
                } else if rel == "schema.sql" || rel.starts_with("seed/") {
                    oracle_files.push(rel);
                } else {
                    return Err(format!("unrecognised file `oracle/{rel}`"));
                }
            }
        }

        Ok(Self {
            number,
            slug: slug.to_owned(),
            dir: dir.to_path_buf(),
            spec,
            patch_files,
            oracle_files,
            oracle_scenarios,
        })
    }

    fn resolve(
        &self, source: &StagedSource, oracle: &mut StagedOracle,
    ) -> Result<ResolvedStep, String> {
        let change = source
            .apply(self)
            .map_err(|error| format!("step {:02} `{}`: {error}", self.number, self.slug))?;
        let oracle = oracle
            .apply(self)
            .map_err(|error| format!("step {:02} `{}`: {error}", self.number, self.slug))?;
        Ok(ResolvedStep {
            number: self.number,
            slug: self.slug.clone(),
            title: self.spec.title.clone(),
            change,
            amend: self.spec.amend.clone(),
            scope: self.spec.scope.clone(),
            crates: self.spec.crates.clone(),
            oracle,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepToml {
    title: String,
    pub input: StepInput,
    pub amend: ExpectedAmend,
    scope: ExpectedScope,
    pub crates: BTreeMap<String, CrateAction>,
    pub oracle: OracleEdits,
}

impl StepToml {
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: RawStepToml = toml::from_str(text).map_err(|error| error.to_string())?;
        Self::try_from(raw)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepInput {
    Patch { remove: BTreeSet<String> },
    Intent { description: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RawVia {
    Patch,
    Intent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawStepToml {
    title: String,
    via: RawVia,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    remove: Vec<String>,
    amend: RawExpectedAmend,
    scope: RawExpectedScope,
    #[serde(default)]
    crates: BTreeMap<String, CrateAction>,
    #[serde(default)]
    oracle: RawOracleEdits,
}

impl TryFrom<RawStepToml> for StepToml {
    type Error = String;

    fn try_from(raw: RawStepToml) -> Result<Self, String> {
        if raw.title.trim().is_empty() {
            return Err("declares an empty `title`".to_owned());
        }
        let input = match raw.via {
            RawVia::Patch => {
                if raw.description.is_some() {
                    return Err("`via = \"patch\"` cannot set `description`".to_owned());
                }
                let remove = into_unique_set(raw.remove, "remove")?;
                for path in &remove {
                    check_relative_path(path, "remove")?;
                }
                StepInput::Patch { remove }
            }
            RawVia::Intent => {
                if !raw.remove.is_empty() {
                    return Err("`via = \"intent\"` cannot set `remove`".to_owned());
                }
                let Some(description) = raw.description.filter(|text| !text.trim().is_empty())
                else {
                    return Err("`via = \"intent\"` requires a non-empty `description`".to_owned());
                };
                StepInput::Intent { description }
            }
        };

        Ok(Self {
            title: raw.title,
            input,
            amend: ExpectedAmend::try_from(raw.amend)?,
            scope: ExpectedScope::try_from(raw.scope)?,
            crates: raw.crates,
            oracle: OracleEdits::try_from(raw.oracle)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrateAction {
    Rewrite,
    Carried,
    Touch,
    Removed,
    Laid,
}

impl Display for CrateAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Rewrite => "rewrite",
            Self::Carried => "carried",
            Self::Touch => "touch",
            Self::Removed => "removed",
            Self::Laid => "laid",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum CountRange {
    Exact(usize),
    Inclusive([usize; 2]),
}

impl CountRange {
    const fn zero() -> Self {
        Self::Exact(0)
    }

    const fn min(self) -> usize {
        match self {
            Self::Exact(n) => n,
            Self::Inclusive([min, _]) => min,
        }
    }

    const fn max(self) -> usize {
        match self {
            Self::Exact(n) => n,
            Self::Inclusive([_, max]) => max,
        }
    }

    const fn is_valid(self) -> bool {
        self.min() <= self.max()
    }

    #[cfg(test)]
    pub fn contains(self, count: usize) -> bool {
        (self.min()..=self.max()).contains(&count)
    }
}

impl Display for CountRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.min() == self.max() {
            write!(f, "{}", self.min())
        } else {
            write!(f, "{}..={}", self.min(), self.max())
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawExpectedAmend {
    #[serde(default = "CountRange::zero")]
    added: CountRange,
    #[serde(default = "CountRange::zero")]
    changed: CountRange,
    #[serde(default = "CountRange::zero")]
    retired: CountRange,
    #[serde(default)]
    stems: Vec<String>,
    #[serde(default)]
    sections: Vec<String>,
    #[serde(default)]
    parts_added: Vec<String>,
    #[serde(default)]
    parts_removed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedAmend {
    pub added: CountRange,
    pub changed: CountRange,
    retired: CountRange,
    stems: BTreeSet<String>,
    sections: BTreeSet<String>,
    parts_added: BTreeSet<String>,
    parts_removed: BTreeSet<String>,
}

impl TryFrom<RawExpectedAmend> for ExpectedAmend {
    type Error = String;

    fn try_from(raw: RawExpectedAmend) -> Result<Self, String> {
        for (field, range) in [
            ("amend.added", raw.added),
            ("amend.changed", raw.changed),
            ("amend.retired", raw.retired),
        ] {
            if !range.is_valid() {
                return Err(format!("`{field}` has min {} > max {}", range.min(), range.max()));
            }
        }
        Ok(Self {
            added: raw.added,
            changed: raw.changed,
            retired: raw.retired,
            stems: into_unique_set(raw.stems, "amend.stems")?,
            sections: into_unique_set(raw.sections, "amend.sections")?,
            parts_added: into_unique_set(raw.parts_added, "amend.parts-added")?,
            parts_removed: into_unique_set(raw.parts_removed, "amend.parts-removed")?,
        })
    }
}

impl Display for ExpectedAmend {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "added {}, changed {}, retired {}", self.added, self.changed, self.retired)?;
        if !self.stems.is_empty() {
            write!(f, " in {}", comma_joined(&self.stems))?;
        }
        if !self.sections.is_empty() {
            write!(f, "; sections {}", comma_joined(&self.sections))?;
        }
        if !self.parts_added.is_empty() {
            write!(f, "; +parts {}", comma_joined(&self.parts_added))?;
        }
        if !self.parts_removed.is_empty() {
            write!(f, "; -parts {}", comma_joined(&self.parts_removed))?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawExpectedScope {
    #[serde(default)]
    minimum: Vec<String>,
    #[serde(default)]
    widened: Vec<String>,
    #[serde(default)]
    rebuilt: Vec<String>,
    #[serde(default)]
    retiring: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ExpectedScope {
    minimum: BTreeSet<String>,
    widened: BTreeSet<String>,
    rebuilt: BTreeSet<String>,
    retiring: BTreeSet<String>,
}

impl TryFrom<RawExpectedScope> for ExpectedScope {
    type Error = String;

    fn try_from(raw: RawExpectedScope) -> Result<Self, String> {
        let minimum = into_unique_set(raw.minimum, "scope.minimum")?;
        let widened = into_unique_set(raw.widened, "scope.widened")?;
        let rebuilt = into_unique_set(raw.rebuilt, "scope.rebuilt")?;
        let retiring = into_unique_set(raw.retiring, "scope.retiring")?;
        if let Some(overlap) = minimum.intersection(&widened).next() {
            return Err(format!("`scope.minimum` and `scope.widened` both name `{overlap}`"));
        }
        if !rebuilt.iter().eq(minimum.union(&widened)) {
            return Err(
                "`scope.rebuilt` must equal the union of `scope.minimum` and `scope.widened`"
                    .to_owned(),
            );
        }
        if let Some(overlap) = rebuilt.intersection(&retiring).next() {
            return Err(format!("`scope.rebuilt` and `scope.retiring` both name `{overlap}`"));
        }
        Ok(Self {
            minimum,
            widened,
            rebuilt,
            retiring,
        })
    }
}

impl Display for ExpectedScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "minimum [{}], widened [{}], rebuilt [{}], retiring [{}]",
            comma_joined(&self.minimum),
            comma_joined(&self.widened),
            comma_joined(&self.rebuilt),
            comma_joined(&self.retiring),
        )
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawOracleEdits {
    #[serde(default)]
    remove: Vec<String>,
    #[serde(default)]
    upstream_headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OracleEdits {
    remove: BTreeSet<String>,
    pub upstream_headers: BTreeMap<String, String>,
}

impl TryFrom<RawOracleEdits> for OracleEdits {
    type Error = String;

    fn try_from(raw: RawOracleEdits) -> Result<Self, String> {
        let remove = into_unique_set(raw.remove, "oracle.remove")?;
        for scenario in &remove {
            check_scenario_path(scenario, "oracle.remove")?;
        }
        for key in raw.upstream_headers.keys() {
            if key.trim().is_empty() {
                return Err("`oracle.upstream-headers` has an empty header name".to_owned());
            }
        }
        Ok(Self {
            remove,
            upstream_headers: raw.upstream_headers,
        })
    }
}

struct StagedSource {
    dir: PathBuf,
}

impl StagedSource {
    fn seed(base_fixture: &Path, dir: PathBuf) -> Result<(Self, usize), String> {
        copy_tree(base_fixture, &dir).map_err(|error| format!("copying base fixture: {error}"))?;
        let count =
            list_files(&dir).map_err(|error| format!("listing base fixture: {error}"))?.len();
        Ok((Self { dir }, count))
    }

    fn apply(&self, step: &Step) -> Result<AppliedChange, String> {
        let remove = match &step.spec.input {
            StepInput::Intent { description } => {
                return Ok(AppliedChange::Intent(description.clone()));
            }
            StepInput::Patch { remove } => remove,
        };

        let mut removed = Vec::with_capacity(remove.len());
        for rel in remove {
            let path = self.dir.join(rel);
            if !path.is_file() {
                return Err(format!(
                    "`remove` names `{rel}`, which is absent from the source tree"
                ));
            }
            fs::remove_file(&path).map_err(|error| format!("removing `{rel}`: {error}"))?;
            prune_empty_dirs(path.parent(), &self.dir);
            removed.push(rel.clone());
        }

        let mut added = Vec::new();
        let mut modified = Vec::new();
        let patch_dir = step.dir.join("patch");
        for rel in &step.patch_files {
            if overlay_file(&patch_dir, &self.dir, rel)? {
                modified.push(rel.clone());
            } else {
                added.push(rel.clone());
            }
        }

        check_tree(&self.dir, &format!("{}/source", step.slug))?;
        let total_files =
            list_files(&self.dir).map_err(|error| format!("listing source tree: {error}"))?.len();
        Ok(AppliedChange::Patch(SourceChanges {
            added,
            modified,
            removed,
            total_files,
        }))
    }
}

struct StagedOracle {
    dir: PathBuf,
    upstream_headers: BTreeMap<String, String>,
}

impl StagedOracle {
    fn seed(base_oracle: &Path, dir: PathBuf) -> Result<(Self, usize), String> {
        copy_tree(base_oracle, &dir).map_err(|error| format!("copying base oracle: {error}"))?;
        let count = check_oracle_tree(&dir, "scratch/oracle")?;
        Ok((
            Self {
                dir,
                upstream_headers: BTreeMap::new(),
            },
            count,
        ))
    }

    fn apply(&mut self, step: &Step) -> Result<OracleChanges, String> {
        let scenarios_dir = self.dir.join("scenarios");
        let mut removed = Vec::with_capacity(step.spec.oracle.remove.len());
        for rel in &step.spec.oracle.remove {
            let path = scenarios_dir.join(rel);
            if !path.is_file() {
                return Err(format!(
                    "`oracle.remove` names `{rel}`, which is absent from `oracle/scenarios/`"
                ));
            }
            fs::remove_file(&path)
                .map_err(|error| format!("removing scenario `{rel}`: {error}"))?;
            prune_empty_dirs(path.parent(), &scenarios_dir);
            removed.push(rel.clone());
        }

        let step_oracle = step.dir.join("oracle");
        for rel in &step.oracle_files {
            overlay_file(&step_oracle, &self.dir, rel)?;
        }

        let mut added = Vec::new();
        let mut replaced = Vec::new();
        let step_scenarios = step_oracle.join("scenarios");
        for rel in &step.oracle_scenarios {
            if overlay_file(&step_scenarios, &scenarios_dir, rel)? {
                replaced.push(rel.clone());
            } else {
                added.push(rel.clone());
            }
        }

        for (header, value) in &step.spec.oracle.upstream_headers {
            self.upstream_headers.insert(header.clone(), value.clone());
        }

        let total_scenarios = check_oracle_tree(&self.dir, &format!("{}/oracle", step.slug))?;
        Ok(OracleChanges {
            added,
            replaced,
            removed,
            files: step.oracle_files.clone(),
            upstream_headers: self.upstream_headers.clone(),
            total_scenarios,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppliedChange {
    Patch(SourceChanges),
    Intent(String),
}

impl Display for AppliedChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Patch(source) => write!(
                f,
                "patch (+{} ~{} -{}, {} source files)",
                source.added.len(),
                source.modified.len(),
                source.removed.len(),
                source.total_files,
            ),
            Self::Intent(description) => write!(f, "intent (`{description}`)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChanges {
    pub added: Vec<String>,
    modified: Vec<String>,
    pub removed: Vec<String>,
    total_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleChanges {
    added: Vec<String>,
    replaced: Vec<String>,
    removed: Vec<String>,
    files: Vec<String>,
    pub upstream_headers: BTreeMap<String, String>,
    pub total_scenarios: usize,
}

impl Display for OracleChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "+{} ~{} -{} -> {} scenarios",
            self.added.len(),
            self.replaced.len(),
            self.removed.len(),
            self.total_scenarios,
        )?;
        if !self.files.is_empty() {
            write!(f, " ({})", self.files.join(", "))?;
        }
        if !self.upstream_headers.is_empty() {
            let headers: Vec<String> = self
                .upstream_headers
                .iter()
                .map(|(name, value)| format!("`{name}: {value}`"))
                .collect();
            write!(f, " [upstream {}]", headers.join(", "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStep {
    pub number: usize,
    slug: String,
    title: String,
    pub change: AppliedChange,
    amend: ExpectedAmend,
    scope: ExpectedScope,
    pub crates: BTreeMap<String, CrateAction>,
    pub oracle: OracleChanges,
}

impl Display for ResolvedStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let crates: Vec<String> = self
            .crates
            .iter()
            .map(|(crate_name, action)| format!("`{crate_name}`={action}"))
            .collect();
        write!(
            f,
            "- **{:02} {}** (`{}`): {}\n  - amend: {}\n  - scope: {}\n  - crates: {}\n  - oracle: {}",
            self.number,
            self.title,
            self.slug,
            self.change,
            self.amend,
            self.scope,
            crates.join(", "),
            self.oracle,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedChain {
    name: String,
    base: String,
    target: Target,
    chains: usize,
    baseline_source_files: usize,
    pub baseline_scenarios: usize,
    pub steps: Vec<ResolvedStep>,
}

impl Display for ResolvedChain {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "# {} — evolution chain\n\nbase `{}` · target `{}` · {} chains · {} steps\n",
            self.name,
            self.base,
            self.target.name(),
            self.chains,
            self.steps.len(),
        )?;
        writeln!(
            f,
            "- **00 Baseline**: `specify --apply` + `build` ({} source files, {} oracle scenarios)",
            self.baseline_source_files, self.baseline_scenarios,
        )?;
        for step in &self.steps {
            writeln!(f, "{step}")?;
        }
        Ok(())
    }
}

pub fn evolutions(
    root: &Path, filter: &BTreeSet<String>,
) -> Result<Vec<Evolution>, Box<dyn std::error::Error>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root.join(CASES))? {
        let dir = entry?.path();
        let Some(name) = dir.file_name().and_then(|name| name.to_str()) else { continue };
        if !dir.is_dir() || !dir.join(Evolution::FILE).is_file() {
            continue;
        }
        if !filter.is_empty() && !filter.contains(name) {
            continue;
        }
        found.push(Evolution::read(root, name)?);
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    let unmatched: Vec<&String> = filter
        .iter()
        .filter(|name| !found.iter().any(|evolution| evolution.name == **name))
        .collect();
    if !unmatched.is_empty() {
        return Err(format!("no evolution case under `{CASES}` named {unmatched:?}").into());
    }
    Ok(found)
}

pub fn run_facts(
    root: &Path, eval_dir: &Path, filter: &BTreeSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let evolutions = evolutions(root, filter)?;
    if evolutions.is_empty() {
        return Err(format!("no evolution case under `{CASES}` matches").into());
    }
    for evolution in &evolutions {
        let case_dir = eval_dir.join("facts").join(&evolution.name);
        let resolved = evolution.dry_run(&case_dir.join("scratch"))?;
        let text = resolved.to_string();
        let out = case_dir.join("facts.md");
        fs::write(&out, &text)?;
        println!("{text}");
        eprintln!("eval: `{}` facts at {}", evolution.name, out.display());
    }
    Ok(())
}

fn into_unique_set(items: Vec<String>, label: &str) -> Result<BTreeSet<String>, String> {
    let mut seen = BTreeSet::new();
    for item in items {
        if item.trim().is_empty() {
            return Err(format!("`{label}` holds an empty entry"));
        }
        if seen.contains(&item) {
            return Err(format!("`{label}` names `{item}` twice"));
        }
        seen.insert(item);
    }
    Ok(seen)
}

fn comma_joined(items: &BTreeSet<String>) -> String {
    items.iter().map(String::as_str).collect::<Vec<_>>().join(", ")
}

fn check_relative_path(path: &str, label: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains("//")
    {
        return Err(format!("`{label}` holds invalid path `{path}`"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute() || !parsed.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!(
            "`{label}` path `{path}` must be relative with no `.` or `..` segments"
        ));
    }
    Ok(())
}

fn check_scenario_path(path: &str, label: &str) -> Result<(), String> {
    check_relative_path(path, label)?;
    let Some((group, file)) = path.split_once('/') else {
        return Err(format!("`{label}` entry `{path}` must have shape `<group>/<name>.json`"));
    };
    if group.is_empty()
        || file.contains('/')
        || !Path::new(file).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        return Err(format!("`{label}` entry `{path}` must have shape `<group>/<name>.json`"));
    }
    Ok(())
}

fn check_oracle_spec_file(path: &Path, label: &str) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{label}: {error}"))?;
    Reference::check_spec(&text).map_err(|why| format!("{label}: {why}"))
}

fn check_scenario_file(path: &Path, label: &str) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let parsed: Value = serde_json::from_str(&text).map_err(|error| format!("{label}: {error}"))?;
    Reference::check_scenario(&parsed).map_err(|why| format!("{label} {why}"))
}

fn check_oracle_tree(dir: &Path, label: &str) -> Result<usize, String> {
    check_tree(dir, label)?;
    check_oracle_spec_file(&dir.join(Reference::SPEC_FILE), Reference::SPEC_FILE)?;
    let scenarios =
        list_files(&dir.join("scenarios")).map_err(|error| format!("scenarios/: {error}"))?;
    let mut count = 0;
    for rel in scenarios {
        if !Path::new(&rel).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
            return Err(format!("unrecognised non-JSON file `scenarios/{rel}`"));
        }
        check_scenario_path(&rel, "scenarios")?;
        check_scenario_file(&dir.join("scenarios").join(&rel), &format!("scenarios/{rel}"))?;
        count += 1;
    }
    if count == 0 {
        return Err("holds no scenarios under `scenarios/<group>/`".to_owned());
    }
    Ok(count)
}

fn overlay_file(from_root: &Path, to_root: &Path, rel: &str) -> Result<bool, String> {
    let dest = to_root.join(rel);
    let existed = dest.is_file();
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("creating `{}`: {error}", parent.display()))?;
    }
    fs::copy(from_root.join(rel), &dest).map_err(|error| format!("overlaying `{rel}`: {error}"))?;
    Ok(existed)
}

fn list_files(root: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if skipped_entry(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push(rel);
        }
    }
    Ok(())
}

fn prune_empty_dirs(mut current: Option<&Path>, stop: &Path) {
    while let Some(dir) = current {
        if dir == stop {
            break;
        }
        let empty = fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none());
        if !empty || fs::remove_dir(dir).is_err() {
            break;
        }
        current = dir.parent();
    }
}

//! Reads an `evolution.toml` chain, resolves its step overlays against a base case,
//! and executes live multi-step evolution chains and resumes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use std::{fs, io};

use serde::Deserialize;
use serde_json::Value;

use crate::build::{Build, BuildJob};
use crate::card::short;
use crate::case::{CASES, Case, check_tree};
use crate::fidelity::Target;
use crate::locality::{
    AmendGrade, ScopeGrade, StepComparison, StepLocality, TreeSnapshot, write_locality,
};
use crate::reference::{Comparison, Reference};
use crate::run::{Completion, completions, tail};
use crate::stage::{
    copy_tree, copy_tree_whole, git, repository, retarget, seed_drift, skipped_entry, toml_escaped,
};
use crate::{Rung, Settings, Turns, emery, rust_log};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvolution {
    base: String,
    target: String,
    chains: NonZeroUsize,
    steps: Vec<String>,
}

pub struct Evolution {
    pub name: String,
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

    pub fn adapters(&self) -> BTreeSet<&str> {
        let mut adapters: BTreeSet<&str> = self.base.expected.adapters().collect();
        if self.steps.iter().any(|step| matches!(step.spec.input, StepInput::Intent { .. })) {
            adapters.insert("intent");
        }
        adapters
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
    pub number: usize,
    pub slug: String,
    dir: PathBuf,
    pub spec: StepToml,
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
    pub title: String,
    pub input: StepInput,
    pub amend: ExpectedAmend,
    pub scope: ExpectedScope,
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
    pub retired: CountRange,
    pub stems: BTreeSet<String>,
    pub sections: BTreeSet<String>,
    pub parts_added: BTreeSet<String>,
    pub parts_removed: BTreeSet<String>,
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
pub struct ExpectedScope {
    pub minimum: BTreeSet<String>,
    pub widened: BTreeSet<String>,
    pub rebuilt: BTreeSet<String>,
    pub retiring: BTreeSet<String>,
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

    fn reference(&self, label: &str) -> Result<Reference, String> {
        Reference::at(&self.dir, &format!("{label}/oracle"), self.upstream_headers.clone())
    }

    fn scenario_paths(&self) -> io::Result<BTreeSet<String>> {
        Ok(list_files(&self.dir.join("scenarios"))?.into_iter().collect())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepRange {
    pub start: usize,
    pub end: usize,
}

impl StepRange {
    pub fn parse(text: &str) -> Result<Self, String> {
        let trimmed = text.trim();
        let (start, end) = if let Some((left, right)) = trimmed.split_once("..=") {
            (parse_step_num(left, text)?, parse_step_num(right, text)?)
        } else if let Some((left, right)) = trimmed.split_once("..") {
            (parse_step_num(left, text)?, parse_step_num(right, text)?)
        } else {
            let single = parse_step_num(trimmed, text)?;
            (single, single)
        };
        if start > end {
            return Err(format!("`--steps {text}`: start {start} > end {end}"));
        }
        Ok(Self { start, end })
    }

    pub fn validate(self, max_step: usize) -> Result<Self, String> {
        if self.end > max_step {
            return Err(format!("`--steps {self}` exceeds the case's {max_step} step(s)"));
        }
        Ok(self)
    }
}

impl Display for StepRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}..{:02}", self.start, self.end)
    }
}

fn parse_step_num(part: &str, full: &str) -> Result<usize, String> {
    let trimmed = part.trim();
    if trimmed.is_empty() {
        return Err(format!("`--steps {full}`: expected `A..B` or `K`"));
    }
    trimmed.parse().map_err(|error| format!("`--steps {full}`: `{trimmed}`: {error}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolveResume {
    pub card: String,
    pub chain: usize,
    pub at_step: usize,
    pub drifted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OraclePartition {
    pub carried_passed: usize,
    pub carried_total: usize,
    pub step_passed: usize,
    pub step_total: usize,
    pub note: Option<String>,
}

impl OraclePartition {
    pub fn from_comparison(comparison: &Comparison, step_scenarios: &BTreeSet<String>) -> Self {
        let mut out = Self {
            note: comparison.note().map(str::to_owned),
            ..Self::default()
        };
        for outcome in comparison.outcomes() {
            let name = format!("{}.json", outcome.fixture);
            let is_step =
                step_scenarios.contains(&name) || step_scenarios.contains(&outcome.fixture);
            if is_step {
                out.step_total += 1;
                out.step_passed += usize::from(outcome.passed());
            } else {
                out.carried_total += 1;
                out.carried_passed += usize::from(outcome.passed());
            }
        }
        out
    }

    pub const fn carried_clean(&self) -> bool {
        self.note.is_none() && self.carried_passed == self.carried_total
    }

    pub const fn step_clean(&self) -> bool {
        self.note.is_none() && self.step_passed == self.step_total && self.step_total > 0
    }
}

pub struct StepResult {
    pub step: usize,
    pub tag: String,
    pub title: String,
    pub revision: Option<String>,
    pub amend_exit: Option<i32>,
    pub amend_wall: Duration,
    pub amend_completions: Vec<Completion>,
    pub plan: Option<Value>,
    pub build: Option<Build>,
    pub locality: Option<StepLocality>,
    pub oracle: Option<OraclePartition>,
    pub resumed: bool,
}

impl StepResult {
    pub fn passed(&self) -> bool {
        self.amend_exit == Some(0)
            && self.build.as_ref().is_some_and(Build::landed)
            && self
                .build
                .as_ref()
                .and_then(|b| b.fidelity.as_ref())
                .is_some_and(|f| f.reference.is_some())
    }

    pub fn turns(&self) -> usize {
        self.amend_completions.len() + self.build.as_ref().map_or(0, |b| b.completions.len())
    }

    pub fn wall(&self) -> Duration {
        self.amend_wall + self.build.as_ref().map_or(Duration::ZERO, |b| b.wall)
    }

    fn failed(
        step: usize, tag: String, title: impl Into<String>, exit: Option<i32>, wall: Duration,
        completions: Vec<Completion>,
    ) -> Self {
        Self {
            step,
            tag,
            title: title.into(),
            revision: None,
            amend_exit: exit,
            amend_wall: wall,
            amend_completions: completions,
            plan: None,
            build: None,
            locality: None,
            oracle: None,
            resumed: false,
        }
    }
}

pub struct ChainRun {
    pub n: usize,
    pub steps: Vec<StepResult>,
}

pub struct EvolutionReport {
    pub evolution: Evolution,
    pub target: Target,
    pub chains: Vec<ChainRun>,
    pub resumed: Option<EvolveResume>,
    pub step_range: Option<StepRange>,
}

// Invocation parameters for `run_live`.
pub struct LiveOptions<'a> {
    pub root: &'a Path,
    pub eval_dir: &'a Path,
    pub filter: &'a BTreeSet<String>,
    pub target: Option<Target>,
    pub budget: Option<NonZeroUsize>,
    pub steps: Option<StepRange>,
    pub resume: Option<EvolveResume>,
    pub started: &'a str,
    pub card_name: &'a str,
}

pub fn step_tag(step: usize) -> String {
    format!("step-{step:02}")
}

pub fn step_source_config(base: &Case, reference: &str) -> String {
    let mut config = String::new();
    for source in &base.expected.sources {
        let _ = write!(
            config,
            "[[source]]\nname = \"{}\"\nadapter = \"{reference}\"\n",
            toml_escaped(source.name())
        );
        if source.fixture.is_some() {
            let _ = writeln!(config, "path = \"{}/\"", toml_escaped(source.name()));
            config.push_str("scope = \"whole\"\n");
        } else if let Some(description) = &source.description {
            let _ = writeln!(config, "description = \"{}\"", toml_escaped(description));
        }
        if let Some(rank) = source.rank {
            let _ = writeln!(config, "rank = {rank}");
        }
        config.push('\n');
    }
    config
}

pub fn step_intent_config(reference: &str, description: &str) -> String {
    format!(
        "[[source]]\nname = \"intent\"\nadapter = \"{reference}\"\ndescription = \"{}\"\n",
        toml_escaped(description)
    )
}

pub fn run_live(options: LiveOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let LiveOptions {
        root,
        eval_dir,
        filter,
        target: target_override,
        budget,
        steps: steps_flag,
        resume,
        started,
        card_name,
    } = options;
    let evolutions = evolutions(root, filter)?;
    if evolutions.is_empty() {
        return Err(format!("no evolution case under `{CASES}` matches").into());
    }
    if resume.is_some() && evolutions.len() > 1 {
        return Err("`--evolve --resume` names a single evolution case".into());
    }

    let mut adapters = BTreeSet::new();
    for evolution in &evolutions {
        adapters.extend(evolution.adapters());
    }
    let resolved_target = target_override.unwrap_or_else(|| evolutions[0].target);
    let settings = Settings::from_env(root, &adapters, Some(resolved_target), budget)?;

    let mut reports = Vec::with_capacity(evolutions.len());
    for evolution in evolutions {
        let target = target_override.unwrap_or(evolution.target);
        let max_step = evolution.steps.len();
        let range = match steps_flag {
            Some(r) => r.validate(max_step)?,
            None => StepRange {
                start: resume.as_ref().map_or(0, |r| r.at_step),
                end: max_step,
            },
        };
        if let Some(res) = &resume {
            if res.at_step == 0 || res.at_step > max_step {
                return Err(format!(
                    "`--at-step {}` must be in `1..={max_step}` for `{}`",
                    res.at_step, evolution.name
                )
                .into());
            }
            if range.start < res.at_step {
                return Err(
                    format!("`--steps {range}` starts before `--at-step {}`", res.at_step).into()
                );
            }
        } else if range.start > 0 {
            return Err(format!(
                "`--steps {range}` starts past step 00 without `--resume <card> --at-step {}`",
                range.start
            )
            .into());
        }

        let chain_count = if resume.is_some() || steps_flag.is_some() {
            1
        } else {
            match std::env::var("EVAL_RUNS") {
                Ok(_) => settings.runs.max(1),
                Err(_) => evolution.chains.get(),
            }
        };

        let mut chain_runs = Vec::with_capacity(chain_count);
        for n in 1..=chain_count {
            let chain = if let Some(res) = &resume {
                ChainDriver::resumed(eval_dir, card_name, &evolution, &settings, target, res)?
                    .run_from(res.at_step, range)?
            } else {
                ChainDriver::fresh(eval_dir, card_name, &evolution, &settings, target, n)?
                    .run_all(range)?
            };
            chain_runs.push(chain);
        }

        reports.push(EvolutionReport {
            evolution,
            target,
            chains: chain_runs,
            resumed: resume.clone(),
            step_range: steps_flag,
        });
    }

    let summary = render_card_summary(&settings, &reports, started, card_name);
    let out = eval_dir.join(format!("{card_name}.md"));
    fs::write(&out, &summary)?;
    print!("{summary}");
    eprintln!("eval: evolution scorecard at {}", out.display());
    Ok(())
}

// Per-chain execution state threading the staged project, cumulative source
// and oracle trees, recorded git snapshots, and step results.
struct ChainDriver<'a> {
    chain_n: usize,
    chain_dir: PathBuf,
    evolution: &'a Evolution,
    settings: &'a Settings,
    log_filter: String,
    source: StagedSource,
    oracle: StagedOracle,
    snapshots: BTreeMap<usize, TreeSnapshot>,
    results: Vec<StepResult>,
}

impl<'a> ChainDriver<'a> {
    fn fresh(
        eval_dir: &Path, card_name: &str, evolution: &'a Evolution, settings: &'a Settings,
        target: Target, chain_n: usize,
    ) -> io::Result<Self> {
        let chain_dir =
            eval_dir.join(card_name).join(&evolution.name).join(format!("chain-{chain_n}"));
        eprintln!("eval: `{}` chain {chain_n} staging at {}", evolution.name, chain_dir.display());
        let (source, oracle) = stage_chain_project(&chain_dir, evolution, settings, target)?;
        Ok(Self {
            chain_n,
            chain_dir,
            evolution,
            settings,
            log_filter: chain_rust_log(evolution, settings),
            source,
            oracle,
            snapshots: BTreeMap::new(),
            results: Vec::new(),
        })
    }

    fn resumed(
        eval_dir: &Path, card_name: &str, evolution: &'a Evolution, settings: &'a Settings,
        target: Target, resume: &EvolveResume,
    ) -> io::Result<Self> {
        let source_dir = eval_dir
            .join(&resume.card)
            .join(&evolution.name)
            .join(format!("chain-{}", resume.chain));
        if !source_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no staged chain at `{}`", source_dir.display()),
            ));
        }

        let chain_dir =
            eval_dir.join(card_name).join(&evolution.name).join(format!("chain-{}", resume.chain));
        if chain_dir.exists() {
            fs::remove_dir_all(&chain_dir)?;
        }
        fs::create_dir_all(&chain_dir)?;

        let before_step = resume.at_step - 1;
        let prev_tag = step_tag(before_step);
        let prev_revision = read_recorded_revision(&source_dir, before_step)?;

        copy_prior_chain_state(&source_dir, &chain_dir, resume.at_step)?;
        let repo = repository(&chain_dir, 1);
        let prev_commit = rewind_chain_repo(
            eval_dir,
            card_name,
            &source_dir,
            &repo,
            evolution,
            target,
            resume,
            &prev_revision,
        )?;

        let (source, mut oracle) =
            seed_sources_and_oracle(&chain_dir, evolution, settings, target)?;
        for step in evolution.steps.iter().take(before_step) {
            source.apply(step).map_err(io::Error::other)?;
            oracle.apply(step).map_err(io::Error::other)?;
        }

        let mut results = Vec::with_capacity(evolution.steps.len() + 1);
        let mut snapshots = BTreeMap::new();
        for k in 0..resume.at_step {
            let rev = read_recorded_revision(&chain_dir, k)?;
            let snap = TreeSnapshot::read(&repo, &format!("refs/heads/emery/{rev}"))?;
            snapshots.insert(k, snap);
            results.push(load_recorded_step(&chain_dir, evolution, target, k, &rev));
        }

        eprintln!(
            "eval: `{}` resuming chain {} from `{}` `{prev_tag}` (`{}` @ `{}`)",
            evolution.name,
            resume.chain,
            resume.card,
            short(&prev_revision),
            short(&prev_commit),
        );

        Ok(Self {
            chain_n: resume.chain,
            chain_dir,
            evolution,
            settings,
            log_filter: chain_rust_log(evolution, settings),
            source,
            oracle,
            snapshots,
            results,
        })
    }

    fn run_all(mut self, range: StepRange) -> io::Result<ChainRun> {
        let baseline = self.run_step_zero()?;
        let ok = baseline.passed();
        self.results.push(baseline);
        if !ok {
            return Ok(self.finish());
        }
        self.run_from(1, range)
    }

    fn run_from(mut self, first_step: usize, range: StepRange) -> io::Result<ChainRun> {
        for step in self.evolution.steps.iter().skip(first_step - 1) {
            if step.number > range.end {
                break;
            }
            let result = self.run_amend_step(step)?;
            let ok = result.passed();
            self.results.push(result);
            if !ok {
                break;
            }
        }
        Ok(self.finish())
    }

    fn finish(self) -> ChainRun {
        ChainRun {
            n: self.chain_n,
            steps: self.results,
        }
    }

    fn job<'j>(&'j self, tag: &'j str, revision: &'j str) -> BuildJob<'j> {
        BuildJob {
            project: &self.chain_dir,
            settings: self.settings,
            rust_log: &self.log_filter,
            name: &self.evolution.name,
            n: 1,
            tag,
            revision,
        }
    }

    #[expect(clippy::disallowed_methods, reason = "the eval times live CLI steps on the host")]
    fn run_step_zero(&mut self) -> io::Result<StepResult> {
        let tag = step_tag(0);
        eprintln!("eval: `{}` `{tag}` specifying baseline", self.evolution.name);
        let rung = self
            .settings
            .ladder
            .first()
            .copied()
            .ok_or_else(|| io::Error::other("EVAL_LADDER: no rung"))?;

        let started = Instant::now();
        let output = emery(
            &self.chain_dir,
            self.settings,
            &self.log_filter,
            rung,
            &["specify", "--config", "emery.toml"],
            Turns::Live,
        )?;
        let wall = started.elapsed();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        fs::write(self.chain_dir.join(format!("{tag}.specify.stderr")), stderr.as_bytes())?;
        fs::write(self.chain_dir.join(format!("{tag}.specify.stdout")), stdout.as_bytes())?;
        let specify_completions = completions(&stderr);

        if !output.status.success() {
            eprintln!(
                "eval: `{}` `{tag}` specify failed (exit {:?}):\n{}",
                self.evolution.name,
                output.status.code(),
                tail(&stderr)
            );
            return Ok(StepResult::failed(
                0,
                tag,
                "Baseline",
                output.status.code(),
                wall,
                specify_completions,
            ));
        }

        let envelope: Value = serde_json::from_str(&stdout)?;
        let revision = envelope["revision"]
            .as_str()
            .ok_or_else(|| io::Error::other("`specify` staged no `revision`"))?
            .to_owned();
        let (spec, _design, plan) =
            read_revision_docs(&self.chain_dir, self.settings, &self.log_filter, rung, &tag, true)?;

        self.job(&tag, &revision).apply()?;
        eprintln!(
            "eval: `{}` `{tag}` building baseline `{}` at {}",
            self.evolution.name,
            short(&revision),
            self.settings.build_rung
        );
        let reference = self.oracle.reference(&self.evolution.name).map_err(io::Error::other)?;
        let built = self.job(&tag, &revision).build(Some(&spec), Some(&reference))?;
        let build_stderr = fs::read_to_string(self.chain_dir.join(format!("{tag}.build.stderr")))
            .unwrap_or_default();

        let locality = if built.landed() {
            let repo = repository(&self.chain_dir, 1);
            let snap = TreeSnapshot::read(&repo, &format!("refs/heads/emery/{revision}"))?;
            let loc = StepLocality::baseline(&snap, &build_stderr);
            write_locality(&self.chain_dir, &tag, &loc)?;
            self.snapshots.insert(0, snap);
            Some(loc)
        } else {
            None
        };

        let baseline_step_set = self.oracle.scenario_paths()?;
        let oracle = built
            .fidelity
            .as_ref()
            .and_then(|f| f.reference.as_ref())
            .map(|comp| OraclePartition::from_comparison(comp, &baseline_step_set));

        Ok(StepResult {
            step: 0,
            tag,
            title: "Baseline".to_owned(),
            revision: Some(revision),
            amend_exit: Some(0),
            amend_wall: wall,
            amend_completions: specify_completions,
            plan: Some(plan),
            build: Some(built),
            locality,
            oracle,
            resumed: false,
        })
    }

    #[expect(clippy::disallowed_methods, reason = "the eval times live CLI steps on the host")]
    fn run_amend_step(&mut self, step: &Step) -> io::Result<StepResult> {
        let tag = step_tag(step.number);
        eprintln!("eval: `{}` `{tag}` (`{}`): applying input", self.evolution.name, step.slug);
        let applied = self.source.apply(step).map_err(io::Error::other)?;
        let oracle_changes = self.oracle.apply(step).map_err(io::Error::other)?;
        let config_file = self.prepare_step_config(&tag, &applied)?;

        let rung = self
            .settings
            .ladder
            .first()
            .copied()
            .ok_or_else(|| io::Error::other("EVAL_LADDER: no rung"))?;

        let started = Instant::now();
        let output = emery(
            &self.chain_dir,
            self.settings,
            &self.log_filter,
            rung,
            &["amend", "--config", &config_file],
            Turns::Live,
        )?;
        let amend_wall = started.elapsed();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        fs::write(self.chain_dir.join(format!("{tag}.amend.stderr")), stderr.as_bytes())?;
        fs::write(self.chain_dir.join(format!("{tag}.amend.stdout")), stdout.as_bytes())?;
        let amend_completions = completions(&stderr);

        if !output.status.success() {
            eprintln!(
                "eval: `{}` `{tag}` amend failed (exit {:?}):\n{}",
                self.evolution.name,
                output.status.code(),
                tail(&stderr)
            );
            return Ok(StepResult::failed(
                step.number,
                tag,
                &step.spec.title,
                output.status.code(),
                amend_wall,
                amend_completions,
            ));
        }

        let amend_env: Value = serde_json::from_str(&stdout)?;
        if amend_env["staged"].as_bool() != Some(true) {
            return Err(io::Error::other(format!(
                "`{tag}` of `{}`: `emery amend` staged no amendment",
                self.evolution.name
            )));
        }

        let (amendment, revision) = self.capture_and_seal_amendment(&tag, rung)?;
        let (spec, plan) = write_amended_docs(&self.chain_dir, &tag, &amendment)?;

        eprintln!(
            "eval: `{}` `{tag}` building `{}` at {}",
            self.evolution.name,
            short(&revision),
            self.settings.build_rung
        );
        let reference = self.oracle.reference(&self.evolution.name).map_err(io::Error::other)?;
        let built = self.job(&tag, &revision).build(Some(&spec), Some(&reference))?;
        let locality =
            self.record_step_locality(step, &tag, &revision, &amendment, &plan, &built)?;

        let step_scenarios: BTreeSet<String> =
            oracle_changes.added.iter().chain(oracle_changes.replaced.iter()).cloned().collect();
        let oracle = built
            .fidelity
            .as_ref()
            .and_then(|f| f.reference.as_ref())
            .map(|comp| OraclePartition::from_comparison(comp, &step_scenarios));

        Ok(StepResult {
            step: step.number,
            tag,
            title: step.spec.title.clone(),
            revision: Some(revision),
            amend_exit: Some(0),
            amend_wall,
            amend_completions,
            plan: Some(plan),
            build: Some(built),
            locality,
            oracle,
            resumed: false,
        })
    }

    fn prepare_step_config(&self, tag: &str, applied: &AppliedChange) -> io::Result<String> {
        match applied {
            AppliedChange::Patch(_) => Ok("emery.toml".to_owned()),
            AppliedChange::Intent(description) => {
                let intent_ref = self.settings.stage_component("intent")?;
                let file = format!("{tag}.intent.toml");
                fs::write(
                    self.chain_dir.join(&file),
                    step_intent_config(&intent_ref, description),
                )?;
                Ok(file)
            }
        }
    }

    // Reads the staged `Amendment` via `emery show amendment` before `apply` clears
    // the staging slot, then seals it with `emery apply` and returns `(amendment, revision)`.
    fn capture_and_seal_amendment(&self, tag: &str, rung: Rung) -> io::Result<(Value, String)> {
        let shown = emery(
            &self.chain_dir,
            self.settings,
            &self.log_filter,
            rung,
            &["show", "amendment"],
            Turns::Live,
        )?;
        if !shown.status.success() {
            return Err(io::Error::other(format!(
                "`{tag}` of `{}`: `emery show amendment` failed:\n{}",
                self.evolution.name,
                tail(&String::from_utf8_lossy(&shown.stderr))
            )));
        }
        let shown_env: Value = serde_json::from_slice(&shown.stdout)?;
        let amendment = shown_env["document"].clone();
        fs::write(
            self.chain_dir.join(format!("{tag}.amendment.json")),
            serde_json::to_string_pretty(&amendment)?,
        )?;

        let apply_out = emery(
            &self.chain_dir,
            self.settings,
            &self.log_filter,
            self.settings.build_rung,
            &["apply", "--config", "emery.toml"],
            Turns::Live,
        )?;
        let apply_stderr = String::from_utf8_lossy(&apply_out.stderr);
        fs::write(self.chain_dir.join(format!("{tag}.apply.stderr")), apply_stderr.as_bytes())?;
        fs::write(self.chain_dir.join(format!("{tag}.apply.stdout")), &apply_out.stdout)?;
        if !apply_out.status.success() {
            return Err(io::Error::other(format!(
                "`{tag}` of `{}`: `emery apply` failed:\n{}",
                self.evolution.name,
                tail(&apply_stderr)
            )));
        }
        let apply_env: Value = serde_json::from_slice(&apply_out.stdout)?;
        let revision = apply_env["revision"]
            .as_str()
            .ok_or_else(|| io::Error::other(format!("`{tag}`: `apply` answered no `revision`")))?
            .to_owned();
        Ok((amendment, revision))
    }

    fn record_step_locality(
        &mut self, step: &Step, tag: &str, revision: &str, amendment: &Value, plan: &Value,
        built: &Build,
    ) -> io::Result<Option<StepLocality>> {
        if !built.landed() {
            return Ok(None);
        }
        let build_stderr = fs::read_to_string(self.chain_dir.join(format!("{tag}.build.stderr")))
            .unwrap_or_default();
        let prev_plan = self.results.last().and_then(|r| r.plan.as_ref());
        let amend_grade = AmendGrade::evaluate(amendment, &step.spec.amend);
        let scope_grade = built
            .envelope
            .as_ref()
            .map(|env| ScopeGrade::evaluate(env, Some(plan), prev_plan, &step.spec.scope));

        let repo = repository(&self.chain_dir, 1);
        let curr_snap = TreeSnapshot::read(&repo, &format!("refs/heads/emery/{revision}"))?;
        let locality = if let Some(before) = self.snapshots.get(&(step.number - 1)) {
            let mut round_trip_bases = Vec::new();
            if step.number == self.evolution.steps.len() {
                for base_step in [0, 4] {
                    if let Some(snap) = self.snapshots.get(&base_step) {
                        round_trip_bases.push((base_step, snap));
                    }
                }
            }
            let loc = StepLocality::compare(StepComparison {
                repo: &repo,
                step: step.number,
                slug: &step.slug,
                before,
                after: &curr_snap,
                expected_crates: &step.spec.crates,
                amend: Some(amend_grade),
                scope: scope_grade,
                build_envelope: built.envelope.as_ref(),
                build_stderr: &build_stderr,
                round_trip_bases: &round_trip_bases,
            })?;
            write_locality(&self.chain_dir, tag, &loc)?;
            Some(loc)
        } else {
            None
        };
        self.snapshots.insert(step.number, curr_snap);
        Ok(locality)
    }
}

fn write_amended_docs(
    chain_dir: &Path, tag: &str, amendment: &Value,
) -> io::Result<(Value, Value)> {
    let spec = amendment["spec"].clone();
    let design = amendment["design"].clone();
    let plan = amendment["plan"].clone();
    fs::write(chain_dir.join(format!("{tag}.spec.json")), serde_json::to_string_pretty(&spec)?)?;
    fs::write(
        chain_dir.join(format!("{tag}.design.json")),
        serde_json::to_string_pretty(&design)?,
    )?;
    fs::write(chain_dir.join(format!("{tag}.plan.json")), serde_json::to_string_pretty(&plan)?)?;
    Ok((spec, plan))
}

fn seed_sources_and_oracle(
    chain_dir: &Path, evolution: &Evolution, settings: &Settings, target: Target,
) -> io::Result<(StagedSource, StagedOracle)> {
    let [source] = evolution.base.expected.sources.as_slice() else {
        return Err(io::Error::other("evolution base must have one source"));
    };
    let source_ref = settings.stage_component(&source.adapter)?;
    let (staged_source, _) =
        StagedSource::seed(&evolution.fixture_dir, chain_dir.join(source.name()))
            .map_err(io::Error::other)?;
    let (staged_oracle, _) = StagedOracle::seed(&evolution.oracle_dir, chain_dir.join("oracle"))
        .map_err(io::Error::other)?;
    fs::write(chain_dir.join("emery.toml"), step_source_config(&evolution.base, &source_ref))?;
    retarget(chain_dir, settings, target, 1)?;
    Ok((staged_source, staged_oracle))
}

fn stage_chain_project(
    chain_dir: &Path, evolution: &Evolution, settings: &Settings, target: Target,
) -> io::Result<(StagedSource, StagedOracle)> {
    if chain_dir.exists() {
        fs::remove_dir_all(chain_dir)?;
    }
    fs::create_dir_all(chain_dir)?;
    seed_sources_and_oracle(chain_dir, evolution, settings, target)
}

fn copy_prior_chain_state(source_dir: &Path, chain_dir: &Path, at_step: usize) -> io::Result<()> {
    // Copy `.emery/vcs/repos/run-1` and prior step logs; omit `.emery/storage`
    // so no stale staged item or `current-revision` pointer from a later step
    // survives and `store::current` reads `HEAD` directly.
    let src_emery = source_dir.join(".emery");
    if !src_emery.join("vcs/repos/run-1").is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("`{}` holds no repository at `.emery/vcs/repos/run-1`", source_dir.display()),
        ));
    }
    copy_tree_whole(&src_emery.join("vcs/repos/run-1"), &chain_dir.join(".emery/vcs/repos/run-1"))?;
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if entry.file_type()?.is_file()
            && let Some(num) = parse_step_artifact_num(&name)
            && num < at_step
        {
            fs::copy(entry.path(), chain_dir.join(&file_name))?;
        }
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "repository rewind checks template drift against card paths and prunes later branches"
)]
fn rewind_chain_repo(
    eval_dir: &Path, card_name: &str, source_dir: &Path, repo: &Path, evolution: &Evolution,
    target: Target, resume: &EvolveResume, prev_revision: &str,
) -> io::Result<String> {
    let prev_label = format!("emery/{prev_revision}");
    let prev_commit = git(repo, &["rev-parse", &format!("refs/heads/{prev_label}")])?;
    if target == Target::Omnia {
        let scratch = eval_dir.join(card_name).join(format!("{}.seed", evolution.name));
        match seed_drift(repo, &prev_commit, &scratch)? {
            None => eprintln!(
                "eval: `{}` seed at `{}` holds the component's template",
                evolution.name,
                short(&prev_commit)
            ),
            Some(drift) if !resume.drifted => {
                return Err(io::Error::other(format!(
                    "`{}` seeds step {:02} of `{}` with a tree laid from an earlier template than \
                     the staged component carries:\n{drift}\nadmit with `--drifted` or re-run \
                     from step 00",
                    resume.card, resume.at_step, evolution.name
                )));
            }
            Some(drift) => eprintln!(
                "eval: `{}` resumes a seed admitted by `--drifted`:\n{drift}",
                evolution.name
            ),
        }
    }

    for k in resume.at_step..=evolution.steps.len() {
        if let Ok(later_rev) = read_recorded_revision(source_dir, k)
            && later_rev != prev_revision
        {
            let _ = git(repo, &["branch", "-D", &format!("emery/{later_rev}")]);
        }
    }
    rewind_repo_head(repo, &prev_commit)?;
    Ok(prev_commit)
}

fn read_revision_docs(
    project: &Path, settings: &Settings, log_filter: &str, rung: Rung, tag: &str, staged: bool,
) -> io::Result<(Value, Value, Value)> {
    let mut docs = [Value::Null, Value::Null, Value::Null];
    for (idx, artifact) in ["spec", "design", "plan"].into_iter().enumerate() {
        let mut args = vec!["show", artifact, "--config", "emery.toml"];
        if staged {
            args.push("--staged");
        }
        let shown = emery(project, settings, log_filter, rung, &args, Turns::Live)?;
        let env: Value = serde_json::from_slice(&shown.stdout)?;
        fs::write(
            project.join(format!("{tag}.{artifact}.json")),
            serde_json::to_string_pretty(&env["document"])?,
        )?;
        docs[idx] = env["document"].clone();
    }
    Ok(docs.into())
}

fn chain_rust_log(evolution: &Evolution, settings: &Settings) -> String {
    settings.rust_log.clone().unwrap_or_else(|| rust_log(evolution.adapters()))
}

fn read_recorded_revision(chain_dir: &Path, step: usize) -> io::Result<String> {
    let tag = step_tag(step);
    let stdout = fs::read_to_string(chain_dir.join(format!("{tag}.apply.stdout")))?;
    let env: Value = serde_json::from_str(&stdout)?;
    env["revision"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other(format!("`{tag}.apply.stdout` holds no `revision`")))
}

fn load_recorded_step(
    chain_dir: &Path, evolution: &Evolution, target: Target, step: usize, revision: &str,
) -> StepResult {
    let tag = step_tag(step);
    let title = if step == 0 {
        "Baseline".to_owned()
    } else {
        evolution.steps[step - 1].spec.title.clone()
    };
    let verb = if step == 0 { "specify" } else { "amend" };
    let stderr =
        fs::read_to_string(chain_dir.join(format!("{tag}.{verb}.stderr"))).unwrap_or_default();
    let plan = fs::read_to_string(chain_dir.join(format!("{tag}.plan.json")))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let build = Build::recorded(chain_dir, 1, &tag, target, revision).ok();
    StepResult {
        step,
        tag,
        title,
        revision: Some(revision.to_owned()),
        amend_exit: Some(0),
        amend_wall: Duration::ZERO,
        amend_completions: completions(&stderr),
        plan,
        build,
        locality: None,
        oracle: None,
        resumed: true,
    }
}

fn parse_step_artifact_num(file_name: &str) -> Option<usize> {
    let rest = file_name.strip_prefix("step-")?;
    let (digits, _) = rest.split_once('.')?;
    digits.parse().ok()
}

// Resets `repo`'s `HEAD` (symbolic branch or detached) and working tree to `commit`
// so `store::current` reads `commit` as `HEAD` without touching `.emery/storage`.
pub fn rewind_repo_head(repo: &Path, commit: &str) -> io::Result<()> {
    git(repo, &["reset", "--quiet", "--hard", commit])?;
    Ok(())
}

fn render_card_summary(
    settings: &Settings, reports: &[EvolutionReport], started: &str, card_name: &str,
) -> String {
    let mut out = format!(
        "# Evolution {started}\n\nmodel `{}` · backend {} · emery `{}` · runs under \
         `target/eval/{card_name}/`\n",
        settings.model,
        settings.backend.name(),
        settings.emery.display(),
    );
    for report in reports {
        let _ =
            writeln!(out, "\n## {} (target `{}`)\n", report.evolution.name, report.target.name());
        if let Some(range) = &report.step_range {
            let _ = writeln!(out, "Steps `{range}`.\n");
        }
        if let Some(res) = &report.resumed {
            let _ = writeln!(
                out,
                "Resumed from `{}` chain {} at step {:02}.\n",
                res.card, res.chain, res.at_step
            );
        }
        for chain in &report.chains {
            let _ = writeln!(out, "### Chain {}\n", chain.n);
            let _ = writeln!(
                out,
                "| step | revision | title | amend | scope | carried id | touch violations | oracle carried | oracle step | turns | wall |\n\
                 | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
            );
            for step in &chain.steps {
                let rev_cell = step.revision.as_deref().map_or("—", short);
                let amend_cell = if step.resumed {
                    "resumed".to_owned()
                } else if step.step == 0 {
                    "—".to_owned()
                } else {
                    step.locality.as_ref().and_then(|l| l.amend.as_ref()).map_or_else(
                        || format!("exit {:?}", step.amend_exit),
                        |a| if a.passed() { "ok".to_owned() } else { "FAIL".to_owned() },
                    )
                };
                let scope_cell = if step.resumed {
                    "resumed".to_owned()
                } else if step.step == 0 {
                    "whole".to_owned()
                } else {
                    step.locality.as_ref().and_then(|l| l.scope.as_ref()).map_or_else(
                        || "—".to_owned(),
                        |s| if s.passed() { "ok".to_owned() } else { "FAIL".to_owned() },
                    )
                };
                let carried_cell = step.locality.as_ref().map_or("—", |l| {
                    if step.step == 0 {
                        "—"
                    } else if l.carried_byte_identical() {
                        "yes"
                    } else {
                        "NO"
                    }
                });
                let touch_cell = step
                    .locality
                    .as_ref()
                    .map_or_else(|| "—".to_owned(), |l| l.touch_violations().len().to_string());
                let (ora_carried, ora_step) = step.oracle.as_ref().map_or_else(
                    || ("—".to_owned(), "—".to_owned()),
                    |o| {
                        let carried = if o.carried_clean() {
                            format!("{}/{}", o.carried_passed, o.carried_total)
                        } else {
                            format!("{}/{} FAIL", o.carried_passed, o.carried_total)
                        };
                        let own = if o.step_total == 0 {
                            "0/0".to_owned()
                        } else if o.step_clean() {
                            format!("{}/{}", o.step_passed, o.step_total)
                        } else {
                            format!("{}/{} FAIL", o.step_passed, o.step_total)
                        };
                        (carried, own)
                    },
                );
                let _ = writeln!(
                    out,
                    "| `{}` | `{rev_cell}` | {} | {amend_cell} | {scope_cell} | {carried_cell} | {touch_cell} | \
                     {ora_carried} | {ora_step} | {} | {}s |",
                    step.tag,
                    step.title,
                    step.turns(),
                    step.wall().as_secs(),
                );
            }
        }
    }
    out
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

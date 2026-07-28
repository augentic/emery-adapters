//! Deterministic base-repo scaffold for Omnia guest workspaces.
//!
//! Reads the guest template contract from the exemplar checkout the
//! build's preparation leg places at `target/omnia-exemplar/` inside the
//! lent consumer workspace (`exemplar.yaml` → `templates/guest/
//! manifest.yaml`), then writes every missing tooling target atomically.
//! Fill-only: an existing file is never overwritten, so consumer
//! customizations always stand. A missing or malformed checkout fails
//! closed — the agent must never recreate deterministic files from prose.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use serde::Deserialize;

/// Checkout location the preparation leg maintains, relative to the
/// consumer workspace root.
pub const CHECKOUT_DIR: &str = "target/omnia-exemplar";

/// Project-relative path of the scaffolded publish workflow.
pub const PUBLISH_WORKFLOW: &str = ".github/workflows/publish.yaml";

/// Project-relative path of the scaffolded cargo-vet config.
pub const VET_CONFIG: &str = "supply-chain/config.toml";

const EXEMPLAR_SCHEMA_VERSION: u32 = 1;
const MANIFEST_SCHEMA_VERSION: u32 = 3;

/// Why a scaffold pass could not run.
#[derive(Debug)]
pub enum Error {
    /// The exemplar checkout is absent, unreadable, or violates the
    /// template contract.
    Checkout(String),
    /// Writing a scaffold target into the consumer workspace failed.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Checkout(detail) => write!(f, "exemplar checkout: {detail}"),
            Self::Io(err) => write!(f, "scaffold write: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// Soft warning when the consumer's Omnia pin differs from the exemplar's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinMismatch {
    /// Exemplar `omnia.version`.
    pub exemplar_version: String,
    /// Exemplar `omnia.rev`.
    pub exemplar_rev: String,
    /// Consumer workspace `omnia` version, when parseable.
    pub consumer_version: Option<String>,
    /// Consumer `[patch.crates-io]` omnia `rev`, when parseable.
    pub consumer_rev: Option<String>,
}

/// Outcome of one [`ensure_missing`] pass.
#[derive(Debug, Default)]
pub struct EnsureReport {
    /// Relative paths written this pass, in manifest order.
    pub written: Vec<String>,
    /// Relative paths already present and left untouched, in manifest order.
    pub skipped: Vec<String>,
    /// `<UPPER_SNAKE>` placeholder tokens the manifest declares (sorted);
    /// seed templates carry them verbatim for the guest writer to fill.
    pub tokens: Vec<String>,
    /// Declared tokens still present as literals in [`PUBLISH_WORKFLOW`].
    pub unfilled_tokens: Vec<String>,
    /// Soft warning when the consumer pin differs from the exemplar contract.
    pub pin_mismatch: Option<PinMismatch>,
}

/// Write every base-repo tooling file absent from `project_root`, sourced
/// from the exemplar checkout at [`CHECKOUT_DIR`].
///
/// # Errors
///
/// [`Error::Checkout`] when the checkout is missing or violates the
/// contract; [`Error::Io`] on the first failed write. Files written
/// before a failure stay in place.
pub fn ensure_missing(project_root: &Path) -> Result<EnsureReport, Error> {
    let checkout = project_root.join(CHECKOUT_DIR);
    let (exemplar, manifest) = load_contract(&checkout)?;

    let mut report = EnsureReport {
        tokens: manifest.tokens.keys().map(|token| format!("<{token}>")).collect(),
        ..EnsureReport::default()
    };
    for entry in &manifest.assemblies.core.files {
        let target = project_root.join(&entry.target);
        if target.exists() {
            report.skipped.push(entry.target.clone());
            continue;
        }
        let source = checkout.join(&entry.source);
        let contents = fs::read_to_string(&source).map_err(|err| {
            Error::Checkout(format!("manifest source `{}` is unreadable: {err}", entry.source))
        })?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        write_atomic(&target, &contents).map_err(Error::Io)?;
        report.written.push(entry.target.clone());
    }
    report.unfilled_tokens = unfilled_publish_tokens(project_root, &report.tokens);
    report.pin_mismatch = pin_mismatch(project_root, &exemplar);
    Ok(report)
}

/// Strictly parse and validate the checkout's template contract.
fn load_contract(checkout: &Path) -> Result<(Exemplar, Manifest), Error> {
    if !checkout.is_dir() {
        return Err(Error::Checkout(format!(
            "no checkout at `{CHECKOUT_DIR}` — the preparation leg must run first"
        )));
    }

    let exemplar: Exemplar = parse_yaml(&checkout.join("exemplar.yaml"))?;
    if exemplar.schema_version != EXEMPLAR_SCHEMA_VERSION {
        return Err(Error::Checkout(format!(
            "exemplar.yaml: unsupported schema-version {} (expected {EXEMPLAR_SCHEMA_VERSION})",
            exemplar.schema_version
        )));
    }
    if exemplar.omnia.rev.is_empty() {
        return Err(Error::Checkout("exemplar.yaml declares no omnia rev".to_string()));
    }
    let manifest_rel = exemplar.templates.manifest.clone();
    let manifest_str = manifest_rel.to_string_lossy();
    if is_unsafe(&manifest_str) {
        return Err(Error::Checkout(format!("unsafe manifest path `{manifest_str}`")));
    }

    let manifest: Manifest = parse_yaml(&checkout.join(&manifest_rel))?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(Error::Checkout(format!(
            "manifest: unsupported schema-version {} (expected {MANIFEST_SCHEMA_VERSION})",
            manifest.schema_version
        )));
    }
    if manifest.assemblies.core.path_mode != "content-only" {
        return Err(Error::Checkout(format!(
            "manifest: unsupported path-mode `{}`",
            manifest.assemblies.core.path_mode
        )));
    }
    for entry in &manifest.assemblies.core.files {
        for (label, path) in [("source", &entry.source), ("target", &entry.target)] {
            if is_unsafe(path) {
                return Err(Error::Checkout(format!("manifest: unsafe {label} path `{path}`")));
            }
        }
        if entry.proof == Proof::Exact && entry.source != entry.target {
            return Err(Error::Checkout(format!(
                "manifest: proof `exact` requires source == target, got `{}` -> `{}`",
                entry.source, entry.target
            )));
        }
    }
    Ok((exemplar, manifest))
}

fn parse_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let text = fs::read_to_string(path)
        .map_err(|err| Error::Checkout(format!("{}: {err}", path.display())))?;
    serde_saphyr::from_str(&text)
        .map_err(|err| Error::Checkout(format!("{}: {err}", path.display())))
}

/// Absolute or parent-traversing paths never belong in the manifest.
fn is_unsafe(path: &str) -> bool {
    Path::new(path).is_absolute() || path.split('/').any(|segment| segment == "..")
}

// A fill-only pass never revisits an existing path, so a write left
// half-complete by a crash would be kept forever; temp + rename keeps
// every scaffolded file whole.
fn write_atomic(target: &Path, contents: &str) -> io::Result<()> {
    let file_name = target.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
    let tmp = target.with_file_name(format!(".{file_name}.scaffold-tmp"));
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, target).inspect_err(|_| {
        drop(fs::remove_file(&tmp));
    })
}

fn unfilled_publish_tokens(project_root: &Path, tokens: &[String]) -> Vec<String> {
    let path = project_root.join(PUBLISH_WORKFLOW);
    let Ok(body) = fs::read_to_string(path) else {
        return Vec::new();
    };
    tokens.iter().filter(|token| body.contains(token.as_str())).cloned().collect()
}

fn pin_mismatch(project_root: &Path, exemplar: &Exemplar) -> Option<PinMismatch> {
    let cargo = project_root.join("Cargo.toml");
    let Ok(text) = fs::read_to_string(cargo) else {
        return None;
    };
    let consumer_version = workspace_dep_version(&text, "omnia");
    let consumer_rev = patch_rev(&text, "omnia");
    let version_differs =
        consumer_version.as_ref().is_some_and(|version| version != &exemplar.omnia.version);
    let rev_differs = consumer_rev.as_ref().is_some_and(|rev| rev != &exemplar.omnia.rev);
    if !version_differs && !rev_differs {
        return None;
    }
    Some(PinMismatch {
        exemplar_version: exemplar.omnia.version.clone(),
        exemplar_rev: exemplar.omnia.rev.clone(),
        consumer_version,
        consumer_rev,
    })
}

/// Best-effort `omnia = "…"` under `[workspace.dependencies]` (or a bare dep table).
fn workspace_dep_version(cargo_toml: &str, name: &str) -> Option<String> {
    let needle = format!("{name} = \"");
    cargo_toml.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed.strip_prefix(&needle).and_then(|rest| rest.split('"').next().map(str::to_string))
    })
}

/// Best-effort `rev = "…"` on a `[patch.crates-io]` `omnia = { … }` line.
fn patch_rev(cargo_toml: &str, name: &str) -> Option<String> {
    let prefix = format!("{name} = {{");
    cargo_toml.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(&prefix) {
            return None;
        }
        trimmed.split("rev = \"").nth(1).and_then(|rest| rest.split('"').next().map(str::to_string))
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Exemplar {
    schema_version: u32,
    omnia: OmniaPin,
    templates: Templates,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OmniaPin {
    version: String,
    #[expect(dead_code, reason = "compatibility contract consumed by the build prompts")]
    repository: String,
    rev: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Templates {
    manifest: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Manifest {
    schema_version: u32,
    tokens: BTreeMap<String, String>,
    assemblies: Assemblies,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Assemblies {
    core: Assembly,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Assembly {
    path_mode: String,
    files: Vec<FileEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEntry {
    source: String,
    target: String,
    proof: Proof,
}

#[derive(PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Proof {
    Exact,
    Seed,
}

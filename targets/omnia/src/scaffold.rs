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
    let manifest = load_contract(&checkout)?;

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
    Ok(report)
}

/// Strictly parse and validate the checkout's template contract.
fn load_contract(checkout: &Path) -> Result<Manifest, Error> {
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
    let manifest_rel = exemplar.templates.manifest;
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
    Ok(manifest)
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
    #[expect(dead_code, reason = "compatibility contract consumed by the build prompts")]
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

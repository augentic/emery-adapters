//! Allowlisted copy of a local `vectis-template` checkout into a consumer project.
//!
//! This module is the tested contract for greenfield tree bootstrap from
//! `$TEMPLATE_DIR`. It does **not** invent Crux/BoltFFI/AGP pins — those
//! travel as bytes from the template. Capability stripping (`VECTIS-OPTIONAL`)
//! stays an agent judgment step after materialize.
//!
//! # Template resolution
//!
//! Default path is [`DEFAULT_RELATIVE_DIR`] (`../vectis-template`) relative to
//! the consumer project root. Override with `VECTIS_TEMPLATE_DIR`. Late-cap
//! and strip guidance stay in `$TEMPLATE_DIR/AGENTS.md` — that file is
//! **not** copied into the consumer.
//!
//! # Allowlist (copied when present)
//!
//! **Root files:** `Makefile`, `Makefile.toml`, `Cargo.toml`, `Cargo.lock`,
//! `rust-toolchain.toml`, `deny.toml`, `README.md`, `.gitignore`.
//!
//! **Root directories:** `shared/`, `iOS/`, `Android/` (including the Gradle
//! wrapper), `supply-chain/`, `.maestro/` (infra plus demo journeys; the
//! agent strips `cap=demo` after copy).
//!
//! # Denylist (never copied)
//!
//! - Entire roots: `.git/`, `.github/`, `web/` (out of scope), `AGENTS.md`,
//!   `.vscode/`, `.pnpm-store/`
//! - Within allowlisted trees: `target/`, `.gradle/`, `.idea/`, `.kotlin/`,
//!   `DerivedData/`, `build/`, `generated/`, `xcuserdata/`, `node_modules/`,
//!   `*.xcodeproj/` (regenerate via `xcodegen` / `make -C iOS generate-project`),
//!   `local.properties`, `.env.local`, `.DS_Store`
//!
//! # Identity substitution
//!
//! A closed set of template identity strings is rewritten so DX Makefiles
//! keep working. Pins are never rewritten except where they share a string
//! with the package id (e.g. `io.augentic.vectisapp` in `boltffi.toml`).

use std::fs;
use std::path::{Path, PathBuf};

use super::{ScaffoldError, validate_app_name};

/// Default `$TEMPLATE_DIR` relative to the consumer project root.
pub const DEFAULT_RELATIVE_DIR: &str = "../vectis-template";

/// Environment override for the local template checkout.
pub const TEMPLATE_DIR_ENV: &str = "VECTIS_TEMPLATE_DIR";

/// Fixed identity strings embedded in [`augentic/vectis-template`](https://github.com/augentic/vectis-template).
pub const TEMPLATE_APP_NAME: &str = "VectisApp";
/// Android / bundle application id in the template.
pub const TEMPLATE_ANDROID_PACKAGE: &str = "io.augentic.vectisapp";

const ROOT_FILES: &[&str] = &[
    "Makefile",
    "Makefile.toml",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "deny.toml",
    "README.md",
    ".gitignore",
];

const ROOT_DIRS: &[&str] = &["shared", "iOS", "Android", "supply-chain", ".maestro"];

const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    ".github",
    "web",
    "target",
    ".gradle",
    ".idea",
    ".kotlin",
    "DerivedData",
    "build",
    "generated",
    "xcuserdata",
    ".pnpm-store",
    "node_modules",
    ".vscode",
];

const SKIP_FILE_NAMES: &[&str] = &[".DS_Store", "local.properties", ".env.local", "AGENTS.md"];

/// DX paths agents must keep aligned with `$TEMPLATE_DIR` (iOS).
///
/// Only paths that exist under a current `vectis-template` checkout are
/// refreshed by [`crate::sync`]; absent template counterparts are reported
/// rather than invented. Pattern-level pin/DX checks are Phase 4 work.
pub const IOS_DX_RELATIVE_PATHS: &[&str] = &["iOS/Makefile", "iOS/project.yml"];

/// DX paths agents must keep aligned with `$TEMPLATE_DIR` (Android).
pub const ANDROID_DX_RELATIVE_PATHS: &[&str] = &[
    "Android/Makefile",
    "Android/settings.gradle.kts",
    "Android/build.gradle.kts",
    "Android/app/build.gradle.kts",
    "Android/shared/build.gradle.kts",
];

/// Consumer identity to substitute for the template's fixed names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// `PascalCase` app / Rust struct / iOS folder name (e.g. `Counter`).
    pub app_name: String,
    /// Android application id (e.g. `com.example.counter`).
    pub android_package: String,
}

impl Identity {
    /// Build an identity after validating the app name shape.
    ///
    /// # Errors
    /// Returns [`ScaffoldError`] when `app_name` is not `PascalCase` ASCII or
    /// `android_package` is empty.
    pub fn new(
        app_name: impl Into<String>, android_package: impl Into<String>,
    ) -> Result<Self, ScaffoldError> {
        let app_name = app_name.into();
        let android_package = android_package.into();
        validate_app_name(&app_name)?;
        if android_package.is_empty() {
            return Err(ScaffoldError::InvalidProject {
                message: "android package must not be empty".into(),
            });
        }
        if android_package.contains('/') || android_package.contains('\\') {
            return Err(ScaffoldError::InvalidProject {
                message: format!(
                    "android package {android_package:?} must be a dotted id (e.g. \"com.example.counter\")"
                ),
            });
        }
        Ok(Self {
            app_name,
            android_package,
        })
    }

    fn package_path(&self) -> String {
        self.android_package.replace('.', "/")
    }
}

/// Summary of one materialize run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Template directory that was copied from.
    pub template_dir: PathBuf,
    /// Destination project root.
    pub dest_dir: PathBuf,
    /// Relative destination paths written, in walk order.
    pub files: Vec<String>,
}

/// Resolve `$TEMPLATE_DIR` from `VECTIS_TEMPLATE_DIR` or [`DEFAULT_RELATIVE_DIR`]
/// under `anchor` (typically the consumer project root).
#[must_use]
pub fn resolve_dir(anchor: &Path) -> Option<PathBuf> {
    if let Ok(raw) = std::env::var(TEMPLATE_DIR_ENV) {
        let path = PathBuf::from(raw);
        return path.is_dir().then_some(path);
    }
    let candidate = anchor.join(DEFAULT_RELATIVE_DIR);
    candidate.is_dir().then_some(candidate)
}

/// Copy the allowlisted template tree into `dest_dir` with identity substitution.
///
/// # Errors
/// Returns [`ScaffoldError`] when the template is missing required roots, a
/// destination path already exists, or I/O fails.
pub fn run(
    template_dir: &Path, dest_dir: &Path, identity: &Identity,
) -> Result<Report, ScaffoldError> {
    if !template_dir.is_dir() {
        return Err(ScaffoldError::InvalidProject {
            message: format!(
                "template directory not found at {} (clone https://github.com/augentic/vectis-template.git or set {TEMPLATE_DIR_ENV})",
                template_dir.display()
            ),
        });
    }
    ensure_template_shape(template_dir)?;

    if !dest_dir.exists() {
        fs::create_dir_all(dest_dir)?;
    }

    let mut planned: Vec<(PathBuf, PathBuf)> = Vec::new();
    for name in ROOT_FILES {
        let src = template_dir.join(name);
        if !src.is_file() {
            continue;
        }
        let rel = map_relative_path(name, identity);
        planned.push((src, dest_dir.join(&rel)));
    }
    for name in ROOT_DIRS {
        let src_root = template_dir.join(name);
        if !src_root.is_dir() {
            return Err(ScaffoldError::InvalidProject {
                message: format!(
                    "template is missing required directory {} under {}",
                    name,
                    template_dir.display()
                ),
            });
        }
        collect_tree(&src_root, template_dir, dest_dir, identity, &mut planned)?;
    }

    for (_, dest) in &planned {
        if dest.exists() {
            return Err(ScaffoldError::InvalidProject {
                message: format!(
                    "refusing to overwrite existing file at {} (materialize into an empty project tree)",
                    dest.display()
                ),
            });
        }
    }

    let mut files = Vec::with_capacity(planned.len());
    for (src, dest) in &planned {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_one(src, dest, identity)?;
        let rel = dest.strip_prefix(dest_dir).unwrap_or(dest).to_string_lossy().replace('\\', "/");
        files.push(rel);
    }

    Ok(Report {
        template_dir: template_dir.to_path_buf(),
        dest_dir: dest_dir.to_path_buf(),
        files,
    })
}

fn ensure_template_shape(template_dir: &Path) -> Result<(), ScaffoldError> {
    for name in ["Cargo.toml", "shared", "iOS", "Android"] {
        let path = template_dir.join(name);
        if !path.exists() {
            return Err(ScaffoldError::InvalidProject {
                message: format!(
                    "template at {} is missing {name} (not a vectis-template checkout?)",
                    template_dir.display()
                ),
            });
        }
    }
    Ok(())
}

fn collect_tree(
    dir: &Path, template_dir: &Path, dest_dir: &Path, identity: &Identity,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), ScaffoldError> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type()?.is_dir() {
            if skip_dir(&name) {
                continue;
            }
            collect_tree(&path, template_dir, dest_dir, identity, out)?;
            continue;
        }
        if skip_file(&name) {
            continue;
        }
        let rel = path.strip_prefix(template_dir).map_err(|err| ScaffoldError::Internal {
            message: format!(
                "path {} is not under template {}: {err}",
                path.display(),
                template_dir.display()
            ),
        })?;
        let rel = rel.to_string_lossy().replace('\\', "/");
        let mapped = map_relative_path(&rel, identity);
        out.push((path, dest_dir.join(mapped)));
    }
    Ok(())
}

fn skip_dir(name: &str) -> bool {
    SKIP_DIR_NAMES.contains(&name) || name.ends_with(".xcodeproj")
}

fn skip_file(name: &str) -> bool {
    SKIP_FILE_NAMES.contains(&name)
}

/// Rewrite template-relative path segments for the consumer identity.
#[must_use]
pub fn map_relative_path(rel: &str, identity: &Identity) -> String {
    let pkg_path = identity.package_path();
    let mut out = rel.replace('\\', "/");
    out = out.replace(&TEMPLATE_ANDROID_PACKAGE.replace('.', "/"), &pkg_path);
    out = out.replace(&format!("iOS/{TEMPLATE_APP_NAME}/"), &format!("iOS/{}/", identity.app_name));
    let swift_leaf = format!("{TEMPLATE_APP_NAME}.swift");
    if out.ends_with(&swift_leaf) {
        let prefix_len = out.len() - swift_leaf.len();
        out = format!("{}{}.swift", &out[..prefix_len], identity.app_name);
    }
    out
}

fn copy_one(src: &Path, dest: &Path, identity: &Identity) -> Result<(), ScaffoldError> {
    let bytes = fs::read(src)?;
    match std::str::from_utf8(&bytes) {
        Ok(text) => {
            let rewritten = substitute_identity(text, identity);
            fs::write(dest, rewritten)?;
        }
        Err(_) => {
            fs::write(dest, bytes)?;
        }
    }
    // Preserve executable bit on gradlew / scripts when the source has it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(src)?.permissions().mode();
        if mode & 0o111 != 0 {
            let mut perms = fs::metadata(dest)?.permissions();
            perms.set_mode(mode);
            fs::set_permissions(dest, perms)?;
        }
    }
    Ok(())
}

/// Apply the closed identity substitution set to UTF-8 file contents.
#[must_use]
pub fn substitute_identity(input: &str, identity: &Identity) -> String {
    let pkg_path = identity.package_path();
    let app = identity.app_name.as_str();
    let mut out = input.to_string();
    // Longest / most specific first so prefixes do not double-replace.
    out = out.replace(TEMPLATE_ANDROID_PACKAGE, &identity.android_package);
    out = out.replace(&TEMPLATE_ANDROID_PACKAGE.replace('.', "/"), &pkg_path);
    out = out.replace("VectisApp_iOSApp", &format!("{app}_iOSApp"));
    out = out.replace("VectisApp-iOS", &format!("{app}-iOS"));
    out = out.replace("VectisTheme", &format!("{app}Theme"));
    out = out.replace(TEMPLATE_APP_NAME, app);
    out = out.replace("rootProject.name = \"Vectis\"", &format!("rootProject.name = \"{app}\""));
    out = out.replace(
        "<string name=\"app_name\">Vectis</string>",
        &format!("<string name=\"app_name\">{app}</string>"),
    );
    out
}

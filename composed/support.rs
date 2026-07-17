//! Support for composed-deployment tests: cargo target-dir discovery,
//! subprocess `cargo`, and deployment-manifest rendering.

#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, ensure};

/// One guest entry in a deployment manifest.
#[derive(Debug, Clone)]
pub struct Guest {
    /// Manifest guest id (`<axis>:<name>`).
    pub id: String,
    /// Path to the built wasm32-wasip2 component.
    pub wasm: PathBuf,
    /// Peer interfaces this guest imports.
    pub link: Vec<String>,
    /// HTTP route prefix, when the guest exposes one.
    pub route: Option<String>,
}

/// Render a deployment manifest over `guests` with one writable `"."` mount.
#[must_use]
pub fn manifest(guests: &[Guest], mount: &Path) -> String {
    use std::fmt::Write as _;

    // Writing into a `String` is infallible here.
    let mut doc = String::new();
    for guest in guests {
        let _ = write!(
            doc,
            "[[guest]]\nid = \"{}\"\nsource.path = \"{}\"\n",
            guest.id,
            guest.wasm.display()
        );
        if !guest.link.is_empty() {
            let links =
                guest.link.iter().map(|link| format!("\"{link}\"")).collect::<Vec<_>>().join(", ");
            let _ = writeln!(doc, "link = [{links}]");
        }
        doc.push('\n');
    }
    let _ =
        write!(doc, "[[mount]]\nname = \".\"\npath = \"{}\"\nwritable = true\n\n", mount.display());
    for guest in guests {
        if let Some(route) = &guest.route {
            let _ =
                write!(doc, "[[route.http]]\nprefix = \"{route}\"\nguest = \"{}\"\n\n", guest.id);
        }
    }
    doc.push_str("[transport]\ndefault = \"in-process\"\n");
    doc
}

/// Cargo target dir for the calling test binary (`<target>/<profile>/deps/<exe>`).
///
/// # Errors
///
/// Returns an error when the current executable path cannot be read or is too shallow.
pub fn target_dir() -> Result<PathBuf> {
    let test_exe = std::env::current_exe().context("test executable has a path")?;
    let dir =
        test_exe.ancestors().nth(3).context("test exe sits at <target>/<profile>/deps/<exe>")?;
    Ok(dir.to_path_buf())
}

/// Run one cargo invocation against the workspace at `root`.
///
/// # Errors
///
/// Returns an error when cargo cannot be spawned or exits non-zero.
pub fn cargo(args: &[&str], root: &Path, target: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .env("CARGO_TARGET_DIR", target)
        .args(args)
        .current_dir(root)
        .status()
        .context("spawning cargo")?;
    ensure!(status.success(), "cargo {} failed with {status}", args.join(" "));
    Ok(())
}

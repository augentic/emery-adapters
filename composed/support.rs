//! Host-side support for the composed-deployment tests (`composed.rs`):
//! cargo-target-dir discovery, a subprocess `cargo` runner, and
//! deployment-manifest rendering over [`Guest`] entries. The omnia
//! runtime assembly itself stays with the test, which deploys
//! in-process via `omnia-testkit`.

#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, ensure};

/// One guest entry in a deployment manifest: its id, built component path,
/// the peer interfaces it links, and the HTTP route it serves.
#[derive(Debug, Clone)]
pub struct Guest {
    /// Manifest guest id (`<axis>:<name>`).
    pub id: String,
    /// Path to the built wasm32-wasip2 component.
    pub wasm: PathBuf,
    /// Peer interfaces this guest imports (the manifest `link = [...]`).
    pub link: Vec<String>,
    /// HTTP route prefix served by this guest, when it exposes one.
    pub route: Option<String>,
}

/// Render a deployment manifest over `guests` with one writable `"."`
/// mount at `mount`, an HTTP route per guest that declares one, and
/// in-process transport.
#[must_use]
pub fn manifest(guests: &[Guest], mount: &Path) -> String {
    use std::fmt::Write as _;

    // Writing into a `String` is infallible; the results are discarded.
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

/// The cargo target dir the calling test binary was built into (testkit's
/// convention: the test exe sits at `<target>/<profile>/deps/<exe>`).
///
/// # Errors
///
/// Returns an error when the current executable path cannot be read or is
/// too shallow to hold the expected layout.
pub fn target_dir() -> Result<PathBuf> {
    let test_exe = std::env::current_exe().context("test executable has a path")?;
    let dir =
        test_exe.ancestors().nth(3).context("test exe sits at <target>/<profile>/deps/<exe>")?;
    Ok(dir.to_path_buf())
}

/// Run one cargo invocation against the workspace at `root`, building into
/// `target`.
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

//! Stages a case as a project of its own and drives the host's git over what a run leaves.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use serde_json::Value;

use crate::Settings;
use crate::build::Tree;
use crate::case::Case;
use crate::fidelity::Target;
use crate::run::{Run, accepted, completions, surveyed, unreached};

// Copies each source's fixture into a project of its own beneath `dir` —
// `emery` mounts its invocation directory, so the tree must sit inside it,
// and the engine's state and the repositories its runs seal and build into
// stay with the case — and each adapter's component into the binary's store
// under the `eval:` reference the project names. Each source's tree sits
// under its name; a `--build` run adds its `[target]` table before it runs.
pub fn stage(dir: &Path, case: &Case, settings: &Settings) -> io::Result<PathBuf> {
    let project = dir.join(&case.name);
    if project.exists() {
        fs::remove_dir_all(&project)?;
    }
    fs::create_dir_all(&project)?;
    let mut config = String::new();
    for source in &case.expected.sources {
        let reference = settings.stage_component(&source.adapter)?;
        let _ = write!(
            config,
            "[[source]]\nname = \"{}\"\nadapter = \"{reference}\"\n",
            toml_escaped(source.name())
        );
        match (&source.fixture, &source.description) {
            (Some(fixture), _) => {
                copy_tree(&settings.root.join(fixture), &project.join(source.name()))?;
                let _ = writeln!(config, "path = \"{}/\"", toml_escaped(source.name()));
            }
            (None, Some(description)) => {
                let _ = writeln!(config, "description = \"{}\"", toml_escaped(description));
            }
            (None, None) => {}
        }
        if let Some(rank) = source.rank {
            let _ = writeln!(config, "rank = {rank}");
        }
        config.push('\n');
    }
    fs::write(project.join("emery.toml"), config)?;

    Ok(project)
}

// The `Adapter:` trailer a sealed empty slice carries; the engine's resume
// reads the trailer for its presence alone.
pub const TARGET_NAME: &str = "target";

// The repository the engine creates for run `n` of a case, under `[target]
// name = "run-N"`. One repository per run: `apply` seals a revision over the
// current one where a repository holds it, so a second run sealing into the
// first's would build over the first's tree, and a greenfield build needs a
// repository of its own.
pub fn repository(project: &Path, n: usize) -> PathBuf {
    project.join(format!(".emery/vcs/repos/run-{n}"))
}

// Points the project's `emery.toml` at run `n`'s repository and stages
// `target`'s component in the store. Both targets scaffold a whole tree, so
// no URL is named and `apply` creates the repository beneath the project.
pub fn retarget(project: &Path, settings: &Settings, target: Target, n: usize) -> io::Result<()> {
    let path = project.join("emery.toml");
    let config = fs::read_to_string(&path)?;
    let sources = config.split_once("[target]").map_or(config.as_str(), |(sources, _)| sources);
    let reference = settings.stage_component(target.name())?;
    fs::write(&path, format!("{sources}[target]\nname = \"run-{n}\"\nadapter = \"{reference}\"\n"))
}

fn toml_escaped(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

// One host git over a repository the runner created or reads, under an
// identity of the runner's own so no operator configuration is needed;
// its stdout trimmed, its stderr the error when it fails.
#[expect(
    clippy::disallowed_types,
    reason = "the eval creates and reads its repositories with the host's git"
)]
pub fn git(at: &Path, args: &[&str]) -> io::Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(at)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "emery eval")
        .env("GIT_AUTHOR_EMAIL", "eval@emery.invalid")
        .env("GIT_COMMITTER_NAME", "emery eval")
        .env("GIT_COMMITTER_EMAIL", "eval@emery.invalid")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git {args:?} at `{}`: {}",
            at.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

// What a fixture's checkout may hold that no run reads: dependencies,
// environments, caches, build output, and the engine's state of an earlier
// run; `*.egg-info` by its suffix.
const UNCOPIED: &[&str] = &[
    "node_modules",
    ".git",
    ".emery",
    "dist",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if skipped_entry(&name) {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    Ok(())
}

pub fn skipped_entry(name: &std::ffi::OsStr) -> bool {
    UNCOPIED.iter().any(|skip| name == *skip)
        || name.to_str().is_some_and(|name| name.ends_with(".egg-info"))
}

// The template the staged component embeds, read against the seed a resume
// starts from. A tree `omnia init` laid from an earlier template holds the
// earlier pins in its root manifest and the earlier bodies in its static
// files, and the seed verify that opens a resume is refused by the gate's
// `manifest_drift` or its static-file finding before any slice of the wave
// is built — the template's finding, not the model's. So the drift is read
// here from the commit the resume starts at, through `omnia-template` at
// this workspace's lock, the template `emery_omnia.wasm` built from the
// same lock embeds: the root manifest held outside its two stem lists and
// every static file byte for byte, as the gate holds them. The files are
// read out of the commit into `scratch`, which is removed after; a file the
// commit lacks is left lacking, which the hold reads as drift. The drift as
// the gate would spell it, or none where the seed stands as the template
// spells it.
pub fn seed_drift(repository: &Path, commit: &str, scratch: &Path) -> io::Result<Option<String>> {
    if scratch.exists() {
        fs::remove_dir_all(scratch)?;
    }
    fs::create_dir_all(scratch)?;
    let statics = omnia_template::ENTRIES
        .iter()
        .filter(|entry| entry.role == omnia_template::Role::Static)
        .map(|entry| entry.path);
    for path in statics.chain([omnia_template::Manifest::PATH]) {
        let Some(body) = git_file(repository, commit, path)? else {
            continue;
        };
        let file = scratch.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file, body)?;
    }
    let held = |err: omnia_template::Error| io::Error::other(err.to_string());
    let manifest = omnia_template::Manifest::new(scratch).drift().map_err(held)?;
    let files = omnia_template::drift(scratch).map_err(held)?;
    fs::remove_dir_all(scratch)?;
    let drift: Vec<String> = [manifest, files]
        .iter()
        .filter(|drift| !drift.is_empty())
        .map(ToString::to_string)
        .collect();
    Ok((!drift.is_empty()).then(|| drift.join("\n")))
}

// The bytes of `path` in `commit` of the repository at `at`, whole and
// untrimmed; none where the commit holds no such file.
#[expect(
    clippy::disallowed_types,
    reason = "the eval creates and reads its repositories with the host's git"
)]
fn git_file(at: &Path, commit: &str, path: &str) -> io::Result<Option<Vec<u8>>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(at)
        .args(["cat-file", "-e", &format!("{commit}:{path}")])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(at)
        .args(["show", &format!("{commit}:{path}")])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git show `{commit}:{path}` at `{}`: {}",
            at.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(Some(output.stdout))
}

pub const RUN_1: &str = "run-1";

// A staged project copied under a new card for a build again, with the
// revision its `run-1` committed and that run as the specify left it.
pub struct Restaged {
    pub project: PathBuf,
    pub revision: String,
    pub run: Run,
}

// Copies the staged project of `run-1` of `case` under `card` to the new card:
// the sources, `.emery/storage`, and the run's files less the earlier build's,
// and the repository the earlier build created where `with_repository`, never
// the engine's clones or working copies. Its `emery.toml` gains a `[target]`
// table for this invocation's target, and the run is read back from the files
// the earlier run kept, exactly as `run` read them.
pub fn restaged(
    eval_dir: &Path, card: &str, card_name: &str, case: &Case, settings: &Settings,
    with_repository: bool,
) -> io::Result<Restaged> {
    const N: usize = 1;

    let source = eval_dir.join(card).join(&case.name);
    let project = eval_dir.join(card_name).join(&case.name);
    if !source.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no staged project at `{}`", source.display()),
        ));
    }
    let stdout = fs::read_to_string(source.join(format!("{RUN_1}.stdout")))?;
    let stderr = fs::read_to_string(source.join(format!("{RUN_1}.stderr")))?;
    let envelope: Option<Value> = serde_json::from_str(&stdout).ok();
    let revision = envelope
        .as_ref()
        .and_then(|out| out["revision"].as_str())
        .ok_or_else(|| {
            io::Error::other(format!("`{RUN_1}` of `{}` staged no revision", case.name))
        })?
        .to_owned();

    // the project copied, the engine's working copies and the earlier
    // build's files left behind
    if project.exists() {
        fs::remove_dir_all(&project)?;
    }
    fs::create_dir_all(&project)?;
    for entry in fs::read_dir(&source)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains(".build") || name_str.contains(".fidelity") {
            continue;
        }
        let target = project.join(&name);
        if entry.file_type()?.is_dir() {
            if name == ".emery" {
                copy_tree_whole(&entry.path().join("storage"), &target.join("storage"))?;
                if with_repository {
                    copy_tree_whole(
                        &entry.path().join("vcs/repos/run-1"),
                        &target.join("vcs/repos/run-1"),
                    )?;
                }
            } else {
                copy_tree_whole(&entry.path(), &target)?;
            }
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    let sealed = repository(&source, N);
    let target =
        settings.target.ok_or_else(|| io::Error::other("a build again builds through a target"))?;
    if !with_repository {
        if !sealed.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "`{RUN_1}` of `{}` sealed no revision: no repository at `{}`",
                    case.name,
                    sealed.display()
                ),
            ));
        }
        clone_root(&sealed, &repository(&project, N), &revision)?;
    }
    retarget(&project, settings, target, N)?;

    // the run as the earlier specify left it
    let rung =
        settings.ladder.first().copied().ok_or_else(|| io::Error::other("EVAL_LADDER: no rung"))?;
    let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    tail.reverse();
    let mut run = Run {
        n: 1,
        rung,
        wall: Duration::ZERO,
        exit: Some(0),
        tail: tail.join("\n"),
        waves: envelope.as_ref().and_then(|out| out.get("waves").cloned()),
        evidence: accepted(&stderr),
        completions: completions(&stderr),
        surveyed: surveyed(&stderr),
        unreached: unreached(&stderr),
        spec: None,
        design: None,
        plan: None,
        dead: None,
        build: None,
    };
    for (artifact, slot) in
        [("spec", &mut run.spec), ("design", &mut run.design), ("plan", &mut run.plan)]
    {
        let text = fs::read_to_string(source.join(format!("{RUN_1}.{artifact}.json")))?;
        *slot = Some(serde_json::from_str(&text)?);
    }
    Ok(Restaged {
        project,
        revision,
        run,
    })
}

pub fn clone_root(from: &Path, to: &Path, revision: &str) -> io::Result<()> {
    let repos = to.parent().ok_or_else(|| io::Error::other("a repository sits under a project"))?;
    fs::create_dir_all(repos)?;
    git(
        repos,
        &[
            "clone",
            "--quiet",
            "--single-branch",
            "--no-hardlinks",
            &from.display().to_string(),
            &to.display().to_string(),
        ],
    )?;
    git(to, &["remote", "remove", "origin"])?;
    let subject = git(to, &["log", "-1", "--format=%s", "HEAD"])?;
    if subject != format!("Revision {revision}") {
        return Err(io::Error::other(format!(
            "`{}` holds `{subject}` at its head, not `Revision {revision}`",
            from.display()
        )));
    }
    Ok(())
}

// The tip of `tree` exported whole to `export`, written fresh, and every path
// the tip tracks.
#[expect(clippy::disallowed_types, reason = "the eval unpacks the export with the host's `tar`")]
pub fn export_tip(tree: &Tree, export: &Path) -> io::Result<Vec<String>> {
    let (repo, tip) = tree.at();
    if export.exists() {
        fs::remove_dir_all(export)?;
    }
    fs::create_dir_all(export)?;
    let archive = PathBuf::from(format!("{}.tar", export.display()));
    git(repo, &["archive", "--format=tar", "-o", &archive.display().to_string(), tip])?;
    let untar = std::process::Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(export)
        .output()?;
    fs::remove_file(&archive)?;
    if !untar.status.success() {
        return Err(io::Error::other(format!(
            "tar over the exported tree: {}",
            String::from_utf8_lossy(&untar.stderr).trim()
        )));
    }
    Ok(git(repo, &["ls-tree", "-r", "--name-only", tip])?.lines().map(str::to_owned).collect())
}

// A copy of a directory tree skipping nothing, for a staged project's
// sources.
fn copy_tree_whole(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree_whole(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

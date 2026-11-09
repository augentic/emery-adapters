//! The project tree: its seed, and what a build and a verification put in it around each turn.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::Path;

use emery_sdk::target::{self, BuildContext, Report, Verdict, VerifyContext};
use emery_sdk::{Error, Model, bad_request, server_error, tracing};
use web_rendered_layout::{
    BUNDLE_MEDIA_TYPE, CAPTURE_SCRIPT, Facts, FactsFolder, without_structure,
};

use crate::check::{self, ATTEMPTS, Attempts, FEEDBACK, Turn};
use crate::{EXTRACT, REFACTOR, VERIFY};

/// The page's facts as the extraction turn reads them.
pub const FACTS: &str = "layout/facts";
/// The stripped facts and the reference screenshots the refactor turn reads.
pub const REFACTOR_FACTS: &str = "layout/refactor/facts";
/// The extracted page's screenshots, the refactor turn's reference.
pub const REFACTOR_SHOTS: &str = "layout/refactor/ref";
/// The original facts a build leaves for the verification to score against.
pub const ORIGINAL: &str = "layout/original";
/// Where a build turn captures its render.
pub const CAPTURE: &str = "layout/capture";
/// Where a build turn screenshots its render.
pub const SHOTS: &str = "layout/capture-shots";
/// Where a build turn's check writes its score and overlays.
pub const REVIEW: &str = "layout/review";
/// Where the verification captures, scores, and draws its overlays.
pub const VERIFIED: &str = "layout/verify";
/// The page's fonts and images, served at `/facts/assets/`.
pub const ASSETS: &str = "public/facts/assets";
/// The page's code, the directory every capture is stamped over.
pub const SOURCES: &str = "src";
/// Where the extraction keeps any tooling it adds; gone before the refactor.
pub const TOOLS: &str = "tools";

/// The files the seed writes, each held unchanged by every check but `.gitignore`.
pub const SEED: &[(&str, &str)] = &[
    (".gitignore", include_str!("../seed/.gitignore")),
    ("package.json", include_str!("../seed/package.json")),
    ("vite.config.js", include_str!("../seed/vite.config.js")),
    ("index.html", include_str!("../seed/index.html")),
    ("src/main.js", include_str!("../seed/src/main.js")),
    ("eslint.config.js", include_str!("../seed/eslint.config.js")),
    ("stylelint.config.js", include_str!("../seed/stylelint.config.js")),
    ("layout/capture.mjs", CAPTURE_SCRIPT),
];

pub fn init(root: &str) -> Result<(), Error> {
    let root = Path::new(root);
    let mut scaffolded = 0;
    for (path, content) in SEED {
        let file = root.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| bad_request!("creating `{path}`'s directory: {err}"))?;
        }
        match OpenOptions::new().write(true).create_new(true).open(&file) {
            Ok(mut opened) => {
                opened
                    .write_all(content.as_bytes())
                    .map_err(|err| server_error!("writing `{path}`: {err}"))?;
                scaffolded += 1;
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
            Err(err) => return Err(server_error!("scaffolding `{path}`: {err}")),
        }
    }
    tracing::info!(scaffolded, "scaffolded the page's tree");
    Ok(())
}

/// Extracts the page, refactors its best extraction, and leaves the original facts for the verification.
///
/// Before the extraction the tree holds the facts, the assets, and no page
/// code; before the refactor it holds the stripped facts and the best
/// extraction's screenshots, and neither the facts, the extracted code, nor
/// the extraction's tooling and attempts. Each turn's check scores the
/// turn's own capture against the original facts, over at most
/// [`MAX_ATTEMPTS`](check::MAX_ATTEMPTS) attempts, and the turn ends on
/// its best.
pub async fn build<P: Model>(ctx: &BuildContext<'_, P>) -> Result<Report, Error> {
    let root = Path::new(ctx.workspace);
    let original = attached(ctx)?;
    let facts = Facts::read(&original).map_err(|err| bad_request!("the slice's layout: {err}"))?;

    for dir in [
        ORIGINAL,
        VERIFIED,
        FACTS,
        CAPTURE,
        SHOTS,
        REVIEW,
        FEEDBACK,
        ATTEMPTS,
        TOOLS,
        "layout/refactor",
        ASSETS,
    ] {
        remove_dir(root, dir)?;
    }
    clear_sources(root)?;
    write_folder(&original, &root.join(FACTS))?;
    let mut written = Vec::new();
    for path in original.paths().filter(|path| path.starts_with("assets/")) {
        let to = format!("public/facts/{path}");
        write_file(&root.join(&to), original.get(path).unwrap_or_default())?;
        written.push(to);
    }

    tracing::info!("extracting the page");
    let mut extraction = Attempts::new(root, &facts, Turn::Extract);
    let built = target::build(ctx, EXTRACT, |_| extraction.check()).await;
    let extracted = match built {
        Ok(report) => report.written,
        Err(error) if extraction.best().is_some() => {
            tracing::warn!(%error, "the extraction ended before it answered; refactoring its best attempt");
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    if let Some(best) = extraction.restore_best()? {
        tracing::info!(attempt = best.number, pass = best.pass, "refactoring the best extraction");
    }

    let stripped =
        without_structure(&original).map_err(|err| bad_request!("the slice's layout: {err}"))?;
    write_folder(&stripped, &root.join(REFACTOR_FACTS))?;
    copy_tree(&root.join(SHOTS), &root.join(REFACTOR_SHOTS))?;
    for dir in [FACTS, CAPTURE, SHOTS, REVIEW, FEEDBACK, ATTEMPTS, TOOLS] {
        remove_dir(root, dir)?;
    }
    clear_sources(root)?;
    for path in extracted.iter().filter(|path| extraction_tooling(path)) {
        remove_file(root, path)?;
    }

    tracing::info!("refactoring the page");
    let mut refactor = Attempts::new(root, &facts, Turn::Refactor);
    let mut report = target::build(ctx, REFACTOR, |_| refactor.check()).await?;
    if refactor.restore_best()?.is_some() {
        report.written.retain(|path| root.join(path).is_file());
        for path in source_files(root)? {
            if !report.written.contains(&path) {
                report.written.push(path);
            }
        }
    }
    extraction.write_record()?;

    write_folder(&original, &root.join(ORIGINAL))?;
    written.extend(original.paths().map(|path| format!("{ORIGINAL}/{path}")));
    for path in written {
        if !report.written.contains(&path) {
            report.written.push(path);
        }
    }
    Ok(report)
}

/// Captures the integrated tree once and holds the verdict to its score against the original.
pub async fn verify<P: Model>(ctx: &VerifyContext<'_, P>) -> Result<Verdict, Error> {
    let root = Path::new(ctx.workspace);
    let original = FactsFolder::read(&root.join(ORIGINAL)).map_err(|err| {
        bad_request!(
            "the tree holds no original facts at `{ORIGINAL}`, which a build leaves: {err}"
        )
    })?;
    let facts = Facts::read(&original).map_err(|err| bad_request!("`{ORIGINAL}`: {err}"))?;
    target::verify(ctx, VERIFY, |verdict| check::verdict(root, &facts, verdict)).await
}

// The one layout of the slice: the page's facts.
fn attached<P: Model>(ctx: &BuildContext<'_, P>) -> Result<FactsFolder, Error> {
    let layouts = &ctx.slice.layouts;
    let [layout] = layouts.as_slice() else {
        return Err(bad_request!(
            "the slice carries {} layouts; this adapter rebuilds one page from one",
            layouts.len()
        ));
    };
    if layout.media_type != BUNDLE_MEDIA_TYPE {
        return Err(bad_request!(
            "the slice's layout {} is `{}`, not the rendered-layout facts `{BUNDLE_MEDIA_TYPE}`",
            layout.digest,
            layout.media_type
        ));
    }
    FactsFolder::from_bundle(&layout.body)
        .map_err(|err| bad_request!("the slice's layout {}: {err}", layout.digest))
}

// A file the extraction wrote that is neither the page's code, which is
// cleared whole, nor the tree's own: the extraction's tooling.
fn extraction_tooling(path: &str) -> bool {
    !path.starts_with(&format!("{SOURCES}/"))
        && !path.starts_with("layout/")
        && !path.starts_with(&format!("{ASSETS}/"))
        && !SEED.iter().any(|(seeded, _)| *seeded == path)
}

// Every file under `src/` but the seeded entry, as a `/`-separated path from the root.
fn source_files(root: &Path) -> Result<Vec<String>, Error> {
    let mut files = Vec::new();
    let mut pending = vec![root.join(SOURCES)];
    while let Some(dir) = pending.pop() {
        let entries =
            fs::read_dir(&dir).map_err(|err| server_error!("reading {}: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| server_error!("reading {}: {err}", dir.display()))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(below) = path.strip_prefix(root) {
                let below = below.to_string_lossy().replace('\\', "/");
                if below != "src/main.js" {
                    files.push(below);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Removes every file under `src/` but the seeded entry: the page code a turn writes.
///
/// # Errors
///
/// When `src/` cannot be read or a file in it removed.
pub fn clear_sources(root: &Path) -> Result<(), Error> {
    let sources = root.join(SOURCES);
    let entries = match fs::read_dir(&sources) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(server_error!("reading `{SOURCES}`: {err}")),
    };
    for entry in entries {
        let entry = entry.map_err(|err| server_error!("reading `{SOURCES}`: {err}"))?;
        let path = entry.path();
        if entry.file_name() == "main.js" {
            continue;
        }
        let removed =
            if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
        removed.map_err(|err| server_error!("removing {}: {err}", path.display()))?;
    }
    Ok(())
}

/// Removes `dir` under `root` and everything in it, where it stands.
///
/// # Errors
///
/// When it stands and cannot be removed.
pub fn remove_dir(root: &Path, dir: &str) -> Result<(), Error> {
    match fs::remove_dir_all(root.join(dir)) {
        Err(err) if err.kind() != ErrorKind::NotFound => {
            Err(server_error!("removing `{dir}`: {err}"))
        }
        _ => Ok(()),
    }
}

fn remove_file(root: &Path, path: &str) -> Result<(), Error> {
    match fs::remove_file(root.join(path)) {
        Err(err) if err.kind() != ErrorKind::NotFound => {
            Err(server_error!("removing `{path}`: {err}"))
        }
        _ => Ok(()),
    }
}

fn write_folder(folder: &FactsFolder, dir: &Path) -> Result<(), Error> {
    folder.write(dir).map_err(|err| server_error!("writing {}: {err}", dir.display()))
}

/// Writes `body` to `file`, creating its directories.
///
/// # Errors
///
/// When a directory or the file cannot be written.
pub fn write_file(file: &Path, body: &[u8]) -> Result<(), Error> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| server_error!("creating {}: {err}", parent.display()))?;
    }
    fs::write(file, body).map_err(|err| server_error!("writing {}: {err}", file.display()))
}

/// Copies every file beneath `from` to the same place beneath `to`; nothing where `from` is missing.
///
/// # Errors
///
/// When a file cannot be read or written.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), Error> {
    let entries = match fs::read_dir(from) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(server_error!("reading {}: {err}", from.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|err| server_error!("reading {}: {err}", from.display()))?;
        let path = entry.path();
        if path.is_dir() {
            copy_tree(&path, &to.join(entry.file_name()))?;
        } else if path.is_file() {
            let body = fs::read(&path)
                .map_err(|err| server_error!("reading {}: {err}", path.display()))?;
            write_file(&to.join(entry.file_name()), &body)?;
        }
    }
    Ok(())
}

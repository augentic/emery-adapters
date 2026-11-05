//! The project tree: its seed, and what a build and a verification put in it around each turn.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::Path;

use emery_sdk::target::{self, BuildContext, Report, Verdict, VerifyContext};
use emery_sdk::{Error, Model, bad_request, server_error, tracing};
use web_rendered_layout::{
    BUNDLE_MEDIA_TYPE, CAPTURE_SCRIPT, Facts, FactsFolder, without_structure,
};

use crate::check::{self, Turn};
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

/// Extracts the page, refactors it, and leaves the original facts for the verification.
///
/// Before the extraction the tree holds the facts, the assets, and no page
/// code; before the refactor it holds the stripped facts and the extracted
/// page's screenshots, and neither the facts nor the extracted code. Each
/// turn's check scores the turn's own capture against the original facts.
pub async fn build<P: Model>(ctx: &BuildContext<'_, P>) -> Result<Report, Error> {
    let root = Path::new(ctx.workspace);
    let original = attached(ctx)?;
    let facts = Facts::read(&original).map_err(|err| bad_request!("the slice's layout: {err}"))?;

    for dir in [ORIGINAL, VERIFIED, FACTS, CAPTURE, SHOTS, REVIEW, "layout/refactor", ASSETS] {
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
    target::build(ctx, EXTRACT, |_| check::turn(root, &facts, Turn::Extract)).await?;

    let stripped =
        without_structure(&original).map_err(|err| bad_request!("the slice's layout: {err}"))?;
    write_folder(&stripped, &root.join(REFACTOR_FACTS))?;
    copy_dir(&root.join(SHOTS), &root.join(REFACTOR_SHOTS))?;
    for dir in [FACTS, CAPTURE, SHOTS, REVIEW] {
        remove_dir(root, dir)?;
    }
    clear_sources(root)?;

    tracing::info!("refactoring the page");
    let mut report =
        target::build(ctx, REFACTOR, |_| check::turn(root, &facts, Turn::Refactor)).await?;

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

// Every file under `src/` but the seeded entry: the page code a turn writes.
fn clear_sources(root: &Path) -> Result<(), Error> {
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

fn remove_dir(root: &Path, dir: &str) -> Result<(), Error> {
    match fs::remove_dir_all(root.join(dir)) {
        Err(err) if err.kind() != ErrorKind::NotFound => {
            Err(server_error!("removing `{dir}`: {err}"))
        }
        _ => Ok(()),
    }
}

fn write_folder(folder: &FactsFolder, dir: &Path) -> Result<(), Error> {
    folder.write(dir).map_err(|err| server_error!("writing {}: {err}", dir.display()))
}

fn write_file(file: &Path, body: &[u8]) -> Result<(), Error> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| server_error!("creating {}: {err}", parent.display()))?;
    }
    fs::write(file, body).map_err(|err| server_error!("writing {}: {err}", file.display()))
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), Error> {
    let entries =
        fs::read_dir(from).map_err(|err| server_error!("reading {}: {err}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| server_error!("reading {}: {err}", from.display()))?;
        if entry.path().is_file() {
            let body = fs::read(entry.path())
                .map_err(|err| server_error!("reading {}: {err}", entry.path().display()))?;
            write_file(&to.join(entry.file_name()), &body)?;
        }
    }
    Ok(())
}

//! What a turn's candidate is held to: the seed unchanged, the code's own
//! rules, and a fresh capture of it scored against the original facts.

use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use emery_sdk::target::Verdict;
use emery_sdk::{Error, server_error};
use serde_json::Value;
use web_rendered_layout::{Facts, Review, Width, assemble, review, stamped};

use crate::tree::{CAPTURE, REVIEW, SEED, SHOTS, SOURCES, VERIFIED};

/// The build turn a candidate answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    /// One static `src/Page.vue`.
    Extract,
    /// `src/Page.vue` and its components.
    Refactor,
}

// The page errors a finding quotes.
const QUOTED_ERRORS: usize = 3;

/// The findings on a build turn's candidate, empty when it passes.
///
/// The code's rules are checked first, and a capture is scored only once
/// they hold, since a capture of code that breaks them is not the answer.
///
/// # Errors
///
/// When the score or its overlays cannot be written.
pub fn turn(root: &Path, original: &Facts, turn: Turn) -> Result<Vec<String>, Error> {
    let mut findings = seed_findings(root);
    findings.extend(code_findings(root, turn));
    if !findings.is_empty() {
        return Ok(findings);
    }
    let render = match rendered(root, CAPTURE, original) {
        Ok(render) => render,
        Err(found) => return Ok(found),
    };
    if !holds_png(&root.join(SHOTS)) {
        return Ok(vec![format!(
            "`{SHOTS}/` holds no screenshot; capture with `--screenshots {SHOTS}` as references/capture.md says"
        )]);
    }
    let reviewed = match review(original, &render) {
        Ok(reviewed) => reviewed,
        Err(err) => return Ok(vec![format!("the capture in `{CAPTURE}` cannot be scored: {err}")]),
    };
    write_review(root, REVIEW, &reviewed)?;
    Ok(reviewed.findings(|width| overlay_path(REVIEW, width)))
}

/// The findings on a verdict: the score of a fresh capture of the tree, written to `layout/verify/`.
///
/// The verdict passes exactly when the score passes, and a failing
/// verdict's `failures` are the score's findings, each naming its overlay.
///
/// # Errors
///
/// When the score or its overlays cannot be written.
pub fn verdict(root: &Path, original: &Facts, verdict: &Verdict) -> Result<Vec<String>, Error> {
    let mut findings = seed_findings(root);
    if !findings.is_empty() {
        return Ok(findings);
    }
    let capture = format!("{VERIFIED}/capture");
    let render = match rendered(root, &capture, original) {
        Ok(render) => render,
        Err(found) => return Ok(found),
    };
    let reviewed = match review(original, &render) {
        Ok(reviewed) => reviewed,
        Err(err) => return Ok(vec![format!("the capture in `{capture}` cannot be scored: {err}")]),
    };
    write_review(root, VERIFIED, &reviewed)?;
    let passed = reviewed.comparison.pass;
    let expected = reviewed.findings(|width| overlay_path(VERIFIED, width));
    if verdict.passed != passed {
        findings.push(format!(
            "the capture scores {} against the pass mark, so the verdict is `passed: {passed}`",
            reviewed.comparison.lowest()
        ));
    }
    if !passed && verdict.failures != expected {
        let mut lines = String::new();
        for line in &expected {
            let _ = write!(lines, "\n- {line}");
        }
        findings.push(format!(
            "`failures` are the score's findings, each verbatim, in this order:{lines}"
        ));
    }
    Ok(findings)
}

fn seed_findings(root: &Path) -> Vec<String> {
    SEED.iter()
        .filter(|(path, _)| *path != ".gitignore")
        .filter(|(path, seeded)| {
            fs::read_to_string(root.join(path)).ok().as_deref() != Some(*seeded)
        })
        .map(|(path, _)| {
            format!(
                "`{path}` is not as the seed wrote it; put it back as references/tree.md spells it"
            )
        })
        .collect()
}

fn code_findings(root: &Path, turn: Turn) -> Vec<String> {
    let mut findings = Vec::new();
    let mut files = Vec::new();
    if let Err(err) = walk(&root.join(SOURCES), "", &mut files) {
        return vec![format!("`{SOURCES}/` cannot be read: {err}")];
    }
    if !files.iter().any(|(path, _)| path == "Page.vue") {
        findings.push(format!("`{SOURCES}/Page.vue` is missing; it is the page's root component"));
    }
    for (path, body) in &files {
        for (number, line) in body.lines().enumerate() {
            let squeezed: String =
                line.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
            if squeezed.contains("position:absolute") || squeezed.contains("position:fixed") {
                findings.push(format!(
                    "`{SOURCES}/{path}:{}` positions an element out of the flow; lay it out with flex or grid",
                    number + 1
                ));
            }
            if squeezed.contains("layout/") {
                findings.push(format!(
                    "`{SOURCES}/{path}:{}` names `layout/`, which the page never reads",
                    number + 1
                ));
            }
        }
    }
    if turn == Turn::Refactor {
        let components: Vec<_> = files
            .iter()
            .filter(|(path, _)| path.starts_with("components/") && has_extension(path, "vue"))
            .collect();
        if components.is_empty() {
            findings.push(format!("`{SOURCES}/components/` holds no component"));
        }
        for (path, body) in components {
            if body
                .match_indices("<style")
                .any(|(at, _)| !body[at..].split('>').next().unwrap_or_default().contains("scoped"))
            {
                findings.push(format!("`{SOURCES}/{path}` carries a `<style>` that is not scoped"));
            }
        }
    }
    findings
}

// The render a capture under `dir` holds, or why it is not an answer.
fn rendered(root: &Path, dir: &str, original: &Facts) -> Result<Facts, Vec<String>> {
    let capture = root.join(dir);
    let meta = match fs::read(capture.join("meta.json")) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(vec![format!(
                "`{dir}/` holds no capture; capture as references/capture.md says"
            )]);
        }
        Err(err) => return Err(vec![format!("`{dir}/meta.json` cannot be read: {err}")]),
    };
    stamped(&capture, &root.join(SOURCES)).map_err(|err| vec![format!("`{dir}`: {err}")])?;
    let meta: Value = serde_json::from_slice(&meta)
        .map_err(|err| vec![format!("`{dir}/meta.json` is not JSON: {err}")])?;
    if let Some(errors) = meta["page_errors"].as_array().filter(|errors| !errors.is_empty()) {
        let quoted: Vec<String> = errors.iter().take(QUOTED_ERRORS).map(Value::to_string).collect();
        return Err(vec![format!(
            "the page raised {} errors while captured, first {}",
            errors.len(),
            quoted.join("; ")
        )]);
    }
    let folder = assemble(&capture)
        .map_err(|err| vec![format!("the capture in `{dir}` does not assemble: {err}")])?;
    let render = Facts::read(&folder)
        .map_err(|err| vec![format!("the capture in `{dir}` reads as no facts: {err}")])?;
    if !sweeps_alike(&render, original) {
        return Err(vec![format!(
            "the capture in `{dir}` sweeps widths {:?} and heights {:?} at height {}; capture the original's {:?}, {:?} at {}",
            render.width_range(),
            render.height_range(),
            render.height(),
            original.width_range(),
            original.height_range(),
            original.height()
        )]);
    }
    Ok(render)
}

fn write_review(root: &Path, dir: &str, reviewed: &Review) -> Result<(), Error> {
    let out = root.join(dir);
    fs::create_dir_all(&out).map_err(|err| server_error!("creating `{dir}`: {err}"))?;
    let entries = fs::read_dir(&out).map_err(|err| server_error!("reading `{dir}`: {err}"))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("overlay-") && has_extension(&name, "svg") {
            fs::remove_file(entry.path())
                .map_err(|err| server_error!("removing `{dir}/{name}`: {err}"))?;
        }
    }
    let score = serde_json::to_string_pretty(&reviewed.comparison)
        .map_err(|err| server_error!("writing `{dir}/score.json`: {err}"))?;
    fs::write(out.join("score.json"), score + "\n")
        .map_err(|err| server_error!("writing `{dir}/score.json`: {err}"))?;
    for (width, svg) in &reviewed.overlays {
        fs::write(root.join(overlay_path(dir, *width)), svg)
            .map_err(|err| server_error!("writing `{}`: {err}", overlay_path(dir, *width)))?;
    }
    Ok(())
}

#[expect(clippy::float_cmp, reason = "both sweeps are whole pixels read from JSON")]
fn sweeps_alike(render: &Facts, original: &Facts) -> bool {
    render.width_range() == original.width_range()
        && render.height_range() == original.height_range()
        && render.height() == original.height()
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

fn overlay_path(dir: &str, width: Width) -> String {
    format!("{dir}/overlay-{}.svg", width.0)
}

fn holds_png(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| entry.path().extension().is_some_and(|ext| ext == "png"))
    })
}

// Every `.vue`, `.js`, and `.css` file under `dir`, with its path below it.
fn walk(dir: &Path, prefix: &str, files: &mut Vec<(String, String)>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.file_type()?.is_dir() {
            walk(&entry.path(), &format!("{path}/"), files)?;
        } else if ["vue", "js", "css"].iter().any(|ext| has_extension(&name, ext)) {
            files.push((path, fs::read_to_string(entry.path())?));
        }
    }
    Ok(())
}

//! Experiment / non-production: the rendered-layout facts of a web page, from the command line.
//!
//! ```text
//! web-layout-capture capture --url URL --out DIR [--screenshots DIR] [--stamp DIR]
//!     [--min N] [--max N] [--height N] [--height-min N] [--height-max N] [--tools DIR]
//! web-layout-capture score --original DIR --render DIR --out DIR
//! web-layout-capture draw --facts DIR --out DIR
//! ```
//!
//! `capture` drives a browser over the page served at URL through
//! `tools/web-layout-capture/capture.mjs` and replaces DIR with the facts
//! built from what it recorded. `score` writes `score.json` and the review
//! overlays, and exits 1 when the render does not pass. `draw` writes a
//! wireframe of the facts at each review width.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
#[expect(
    clippy::disallowed_types,
    reason = "the capture drives a browser through the host's node"
)]
use std::process::Command;
use std::process::ExitCode;

use web_rendered_layout::{Facts, FactsFolder, REVIEW_WIDTHS, assemble, draw, review};

// The capture script and its pinned Playwright, beside this workspace's crates.
const TOOLS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/web-layout-capture");

// The range options `capture` hands the script unchanged.
const RANGE_OPTIONS: [&str; 5] = ["min", "max", "height", "height-min", "height-max"];

const USAGE: &str = "usage:
  web-layout-capture capture --url URL --out DIR [--screenshots DIR] [--stamp DIR]
      [--min N] [--max N] [--height N] [--height-min N] [--height-max N] [--tools DIR]
  web-layout-capture score --original DIR --render DIR --out DIR
  web-layout-capture draw --facts DIR --out DIR";

#[expect(clippy::print_stderr, reason = "the command reports its errors")]
#[expect(clippy::disallowed_methods, reason = "a native operator tool, not a guest")]
fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let command = args.next();
    let outcome = match command.as_deref() {
        Some("capture") => Options::parse(args, &CAPTURE).and_then(|options| capture(&options)),
        Some("score") => Options::parse(args, &SCORE).and_then(|options| score(&options)),
        Some("draw") => Options::parse(args, &DRAW).and_then(|options| wireframes(&options)),
        _ => Err(USAGE.to_owned()),
    };
    match outcome {
        Ok(code) => code,
        Err(message) => {
            eprintln!("web-layout-capture: {message}");
            ExitCode::from(2)
        }
    }
}

// The options a command takes: required, then optional.
struct Accepts {
    required: &'static [&'static str],
    optional: &'static [&'static str],
}

const CAPTURE: Accepts = Accepts {
    required: &["url", "out"],
    optional: &[
        "screenshots",
        "stamp",
        "tools",
        "min",
        "max",
        "height",
        "height-min",
        "height-max",
    ],
};
const SCORE: Accepts = Accepts {
    required: &["original", "render", "out"],
    optional: &[],
};
const DRAW: Accepts = Accepts {
    required: &["facts", "out"],
    optional: &[],
};

struct Options(BTreeMap<String, String>);

impl Options {
    fn parse(mut args: impl Iterator<Item = String>, accepts: &Accepts) -> Result<Self, String> {
        let mut options = BTreeMap::new();
        while let Some(flag) = args.next() {
            let name = flag
                .strip_prefix("--")
                .filter(|name| accepts.required.contains(name) || accepts.optional.contains(name))
                .ok_or_else(|| format!("unknown option `{flag}`\n{USAGE}"))?;
            let value = args.next().ok_or_else(|| format!("`{flag}` needs a value"))?;
            if options.insert(name.to_owned(), value).is_some() {
                return Err(format!("`{flag}` is given twice"));
            }
        }
        if let Some(missing) = accepts.required.iter().find(|name| !options.contains_key(**name)) {
            return Err(format!("`--{missing}` is required\n{USAGE}"));
        }
        Ok(Self(options))
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    fn path(&self, name: &str) -> PathBuf {
        PathBuf::from(self.get(name).unwrap_or_default())
    }
}

#[expect(clippy::disallowed_types, reason = "the capture drives a browser through the host's node")]
fn capture(options: &Options) -> Result<ExitCode, String> {
    let tools = options.get("tools").map_or_else(|| PathBuf::from(TOOLS), PathBuf::from);
    if !tools.join("node_modules/playwright").is_dir() {
        return Err(format!(
            "Playwright is not installed in {}; run `npm ci --ignore-scripts` there",
            tools.display()
        ));
    }
    let out = options.path("out");
    let raw = out.with_extension("raw");
    let mut node = Command::new("node");
    node.arg(tools.join("capture.mjs"))
        .args(["--url", options.get("url").unwrap_or_default()])
        .arg("--out")
        .arg(&raw);
    for name in ["screenshots", "stamp"].iter().chain(&RANGE_OPTIONS) {
        if let Some(value) = options.get(name) {
            node.arg(format!("--{name}")).arg(value);
        }
    }
    let status = node.status().map_err(|err| format!("running node: {err}"))?;
    if !status.success() {
        return Err(format!(
            "the capture failed ({status}); its raw output is in {}",
            raw.display()
        ));
    }
    let folder = assemble(&raw).map_err(|err| err.to_string())?;
    Facts::read(&folder).map_err(|err| format!("the assembled facts do not read: {err}"))?;
    replace(&out)?;
    folder.write(&out).map_err(|err| err.to_string())?;
    fs::remove_dir_all(&raw).map_err(|err| format!("removing {}: {err}", raw.display()))?;
    Ok(ExitCode::SUCCESS)
}

#[expect(clippy::print_stdout, reason = "the command reports its findings")]
fn score(options: &Options) -> Result<ExitCode, String> {
    let original = read(&options.path("original"))?;
    let render = read(&options.path("render"))?;
    let out = options.path("out");
    let reviewed = review(&original, &render).map_err(|err| err.to_string())?;
    replace(&out)?;
    let score =
        serde_json::to_string_pretty(&reviewed.comparison).map_err(|err| err.to_string())?;
    write(&out.join("score.json"), &(score + "\n"))?;
    for (width, svg) in &reviewed.overlays {
        write(&out.join(format!("overlay-{}.svg", width.0)), svg)?;
    }
    let findings = reviewed
        .findings(|width| out.join(format!("overlay-{}.svg", width.0)).display().to_string());
    if findings.is_empty() {
        println!("pass: lowest score {}", reviewed.comparison.lowest());
        return Ok(ExitCode::SUCCESS);
    }
    println!("fail:");
    for finding in findings {
        println!("- {finding}");
    }
    Ok(ExitCode::FAILURE)
}

fn wireframes(options: &Options) -> Result<ExitCode, String> {
    let facts = read(&options.path("facts"))?;
    let out = options.path("out");
    fs::create_dir_all(&out).map_err(|err| format!("creating {}: {err}", out.display()))?;
    for width in REVIEW_WIDTHS {
        write(&out.join(format!("wireframe-{}.svg", width.0)), &draw(&facts, width))?;
    }
    Ok(ExitCode::SUCCESS)
}

fn read(dir: &Path) -> Result<Facts, String> {
    FactsFolder::read(dir)
        .and_then(|folder| Facts::read(&folder))
        .map_err(|err| format!("{}: {err}", dir.display()))
}

// Empties `dir`, so nothing from an earlier run survives in it.
fn replace(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|err| format!("removing {}: {err}", dir.display()))?;
    }
    fs::create_dir_all(dir).map_err(|err| format!("creating {}: {err}", dir.display()))
}

fn write(path: &Path, body: &str) -> Result<(), String> {
    fs::write(path, body).map_err(|err| format!("writing {}: {err}", path.display()))
}

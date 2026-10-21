//! `code-to-behaviour --source <root> --entry <file> --screen <slug> --out <dir> [--alias <prefix>=<dir>]...`
//! writes `<out>/behaviour.json` and `<out>/behaviour.unmapped.json`.
//!
//! `code-to-behaviour evidence <behaviour.json>` prints the document as Emery evidence and
//! fails when the claim gate finds a problem.

#![expect(clippy::print_stderr, reason = "operator-facing CLI; stderr is the interface")]

use std::path::PathBuf;
use std::process::ExitCode;

use code_to_behaviour::{Options, evidence, extract};

const USAGE: &str = "usage:
  code-to-behaviour --source <root> --entry <file> --screen <slug> --out <dir> [--alias <prefix>=<dir>]...
  code-to-behaviour evidence <behaviour.json>";

fn parse(args: &[String]) -> Result<(Options, PathBuf), String> {
    let mut source = None;
    let mut entry = None;
    let mut screen = None;
    let mut out = None;
    let mut aliases = Vec::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--source" => source = Some(PathBuf::from(value()?)),
            "--entry" => entry = Some(value()?),
            "--screen" => screen = Some(value()?),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--alias" => {
                let v = value()?;
                let (a, b) = v
                    .split_once('=')
                    .ok_or_else(|| format!("--alias {v}: expected <prefix>=<dir>"))?;
                aliases.push((a.to_string(), b.to_string()));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if aliases.is_empty() {
        aliases.push(("@/".to_string(), "src/".to_string()));
    }
    aliases.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    let options = Options {
        source: source.ok_or("--source is required")?,
        entry: entry.ok_or("--entry is required")?,
        screen: screen.ok_or("--screen is required")?,
        aliases,
    };
    Ok((options, out.ok_or("--out is required")?))
}

fn write(path: &PathBuf, value: &serde_json::Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[expect(clippy::print_stdout, reason = "the evidence document is the command's output")]
fn emit_evidence(path: &str) -> ExitCode {
    let doc = std::fs::read_to_string(path)
        .map_err(|e| format!("{path}: {e}"))
        .and_then(|text| serde_json::from_str(&text).map_err(|e| format!("{path}: {e}")));
    let doc = match doc {
        Ok(doc) => doc,
        Err(reason) => {
            eprintln!("{reason}");
            return ExitCode::FAILURE;
        }
    };
    let evidence = evidence(&doc);
    let findings = evidence.findings();
    match serde_json::to_string_pretty(&evidence) {
        Ok(text) => println!("{text}"),
        Err(reason) => {
            eprintln!("{reason}");
            return ExitCode::FAILURE;
        }
    }
    for finding in &findings {
        eprintln!("{finding}");
    }
    if findings.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

#[expect(clippy::disallowed_methods, reason = "a native operator tool, not a guest")]
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [command, path] = args.as_slice()
        && command == "evidence"
    {
        return emit_evidence(path);
    }
    let (options, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(reason) => {
            eprintln!("{reason}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let output = extract(&options);
    let result = std::fs::create_dir_all(&out)
        .map_err(|e| format!("{}: {e}", out.display()))
        .and_then(|()| write(&out.join(ui_ir_behaviour::BEHAVIOUR), &output.behaviour))
        .and_then(|()| write(&out.join(ui_ir_behaviour::UNMAPPED), &output.unmapped));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("{reason}");
            ExitCode::FAILURE
        }
    }
}

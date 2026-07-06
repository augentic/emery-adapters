//! Eval-driver guest component for the RFC-61 Step 2 decomposition proof.
//!
//! The eval deployment's only `wasi:cli/run` exporter. Imports the
//! `augentic:specify` `workflow` world; Omnia's host-mediated link dispatch
//! routes the `target.build` call to the adapter guest registered under the
//! `adapter-id` first argument. `run` reads the slice's input artifacts from
//! the shared `"."` mount, drives one `build`, and prints the report as one
//! JSON line so the runner can parse the outcome; the exit status carries
//! the report's `status`.
//!
//! Argv (after the program name): `<adapter-id> <slice> <inputs-dir>` —
//! `inputs-dir` is a mount-relative directory whose `*.md` files become the
//! typed inputs, mapped by file stem (`proposal` / `design` / `tasks` /
//! `spec*`; anything else rides as `other`).
#![cfg(target_arch = "wasm32")]

mod generated {
    //! `wit_bindgen::generate!` output for the `workflow` world. The world
    //! only imports (`source` / `target`), so there is no `export!` shim
    //! here; the `wasi:cli/run` export is wired by wasip3 in the crate root.
    #![allow(
        missing_docs,
        unsafe_code,
        clippy::pedantic,
        clippy::nursery,
        reason = "wit-bindgen generated bindings are not hand-maintained; the generated code cannot carry this workspace's lint posture"
    )]

    wit_bindgen::generate!({
        world: "workflow",
        path: "../../wit",
        // Asyncness follows the WIT declarations: the judgment operations
        // are `async func`s (judgment legs await the async `omnia:model`
        // import mid-call) and async-lower; `describe` is a plain `func`
        // (RFC-64) and sync-lowers.
        generate_all,
    });
}

use generated::augentic::specify::target::{self, Input, Report, Status, WorkingTree};
use serde_json::json;

struct CliGuest;
wasip3::cli::command::export!(CliGuest);

impl wasip3::exports::cli::run::Guest for CliGuest {
    async fn run() -> Result<(), ()> {
        let args = wasip3::cli::environment::get_arguments();
        let [_, adapter_id, slice, inputs_dir] = args.as_slice() else {
            eprintln!("usage: <adapter-id> <slice> <inputs-dir>; got {args:?}");
            return Err(());
        };

        let inputs = read_inputs(inputs_dir)?;
        eprintln!("eval: building `{slice}` via `{adapter_id}` with {} inputs", inputs.len());

        let tree = WorkingTree {
            base: "eval".to_string(),
            subpath: None,
        };
        let report = match target::build(adapter_id.clone(), slice.clone(), inputs, tree).await {
            Ok(report) => report,
            Err(error) => {
                eprintln!("build failed: {error:?}");
                return Err(());
            }
        };

        println!("{}", render(&report));
        match report.status {
            Status::Success => Ok(()),
            Status::Failure => Err(()),
        }
    }
}

/// Read every `.md` file under `dir` (sorted by name for a deterministic
/// prompt order) and map each to its typed input by file stem.
fn read_inputs(dir: &str) -> Result<Vec<Input>, ()> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        eprintln!("reading inputs dir `{dir}`: {error}");
    })?;
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();

    let mut inputs = Vec::new();
    for path in paths {
        let body = std::fs::read_to_string(&path).map_err(|error| {
            eprintln!("reading input `{}`: {error}", path.display());
        })?;
        let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
        inputs.push(match stem {
            "proposal" => Input::Proposal(body),
            "design" => Input::Design(body),
            "tasks" => Input::Tasks(body),
            stem if stem.starts_with("spec") => Input::Spec(body),
            _ => Input::Other(body),
        });
    }
    Ok(inputs)
}

/// Render the seam-facing report as one JSON line.
fn render(report: &Report) -> String {
    json!({
        "status": match report.status {
            Status::Success => "success",
            Status::Failure => "failure",
        },
        "findings": report.findings.iter().map(|finding| {
            json!({
                "rule-id": finding.rule_id,
                "severity": format!("{:?}", finding.severity).to_lowercase(),
                "detail": finding.detail,
            })
        }).collect::<Vec<_>>(),
        "outputs": report.outputs.iter().map(|output| {
            json!({
                "platform": format!("{:?}", output.platform).to_lowercase(),
                "path": output.path,
            })
        }).collect::<Vec<_>>(),
        "ui-surface": report.ui_surface.as_ref().map(|surface| json!({ "screens": surface.screens })),
    })
    .to_string()
}

//! Eval guest: the eval deployment's only `wasi:cli/run` exporter, driving
//! one judgment-bearing seam operation per invocation.
//!
//! Imports the `specify:adapter` `workflow` world; Omnia's host-mediated
//! link dispatch routes each call to the adapter guest registered under the
//! `adapter-id` argument. The id passes through verbatim — the operator
//! supplies the axis-qualified form (`source:<name>` / `target:<name>`)
//! matching the guest's registration in the deployment manifest.
//!
//! Argv (after the program name): `<operation> <adapter-id> [args...]`:
//!
//! - `survey <adapter-id>` — source survey; prints the leads as one JSON
//!   array line.
//! - `extract <adapter-id> <inputs-dir>` — source extract; reads the lead
//!   from `<inputs-dir>/lead.json`, an object mirroring the WIT `lead`
//!   record (string `lead` and `synopsis`, optional string-array `topics`
//!   defaulting to empty); prints the evidence as one JSON line.
//! - `guidance <adapter-id>` — target guidance; prints one JSON line
//!   `{"guidance": "…"}`.
//! - `build <adapter-id> <slice> <inputs-dir>` — target build; the
//!   directory's `*.md` files become the typed inputs, mapped by file stem
//!   (`proposal` / `design` / `tasks` / `spec*`; anything else rides as
//!   `other`); prints the report as one JSON line.
//! - `merge <adapter-id> <slice> <inputs-dir>` — target merge; reads the
//!   build delta from `<inputs-dir>/changeset.json`, an object mirroring
//!   the WIT `changeset` record (string `base`, `edits` array of
//!   `{"path": …, "content": …}` objects where an absent or `null`
//!   `content` is a deletion and a present one is a content-addressed
//!   artifact handle); prints the report as one JSON line.
//!
//! Every operation prints its typed answer as one JSON line on stdout with
//! kebab-case keys mirroring the WIT records, and carries its outcome in
//! the exit status: `build` and `merge` exit with the report's `status`,
//! the rest succeed unless the call errors. All paths are relative to the
//! shared `"."` mount.
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
        path: "../wit/deps/specify",
        // Asyncness follows the WIT declarations: the judgment operations
        // are `async func`s (judgment legs await the async `omnia:model`
        // import mid-call) and async-lower; `describe` is a plain `func`
        // (RFC-64) and sync-lowers.
        generate_all,
    });
}

use generated::specify::adapter::source::{self, Backing, Evidence, Lead};
use generated::specify::adapter::target::{self, Input, Report, Status, WorkingTree};
use generated::specify::adapter::types::{Changeset, Edit};
use serde_json::{Value, json};

struct CliGuest;
wasip3::cli::command::export!(CliGuest);

impl wasip3::exports::cli::run::Guest for CliGuest {
    async fn run() -> Result<(), ()> {
        let args = wasip3::cli::environment::get_arguments();
        let [_, operation, tail @ ..] = args.as_slice() else {
            usage(&args);
            return Err(());
        };
        match (operation.as_str(), tail) {
            ("survey", [adapter_id]) => survey(adapter_id).await,
            ("extract", [adapter_id, inputs_dir]) => extract(adapter_id, inputs_dir).await,
            ("guidance", [adapter_id]) => guidance(adapter_id).await,
            ("build", [adapter_id, slice, inputs_dir]) => {
                build(adapter_id, slice, inputs_dir).await
            }
            ("merge", [adapter_id, slice, inputs_dir]) => {
                merge(adapter_id, slice, inputs_dir).await
            }
            _ => {
                usage(&args);
                Err(())
            }
        }
    }
}

fn usage(args: &[String]) {
    eprintln!(
        "usage: <operation> <adapter-id> [args...]; got {args:?}\n\
         operations (<adapter-id> passes through verbatim: the operator supplies\n\
         the axis-qualified id, e.g. `source:typescript` or `target:contracts`):\n\
         \x20 survey   <adapter-id>\n\
         \x20 extract  <adapter-id> <inputs-dir>          reads <inputs-dir>/lead.json\n\
         \x20 guidance <adapter-id>\n\
         \x20 build    <adapter-id> <slice> <inputs-dir>\n\
         \x20 merge    <adapter-id> <slice> <inputs-dir>  reads <inputs-dir>/changeset.json"
    );
}

/// Drive one source `survey`; print the leads as one JSON array line.
async fn survey(adapter_id: &str) -> Result<(), ()> {
    eprintln!("eval: surveying via `{adapter_id}`");
    let leads = source::survey(adapter_id.to_owned()).await.map_err(|error| {
        eprintln!("survey failed: {error:?}");
    })?;
    println!("{}", render_leads(&leads));
    Ok(())
}

/// Drive one source `extract` over the lead read from
/// `<inputs-dir>/lead.json`; print the evidence as one JSON line.
async fn extract(adapter_id: &str, inputs_dir: &str) -> Result<(), ()> {
    let lead = read_lead(inputs_dir)?;
    eprintln!("eval: extracting `{}` via `{adapter_id}`", lead.lead);
    let evidence = source::extract(adapter_id.to_owned(), lead).await.map_err(|error| {
        eprintln!("extract failed: {error:?}");
    })?;
    println!("{}", render_evidence(&evidence));
    Ok(())
}

/// Drive one target `guidance`; print it as one JSON line.
async fn guidance(adapter_id: &str) -> Result<(), ()> {
    eprintln!("eval: fetching guidance via `{adapter_id}`");
    let guidance = target::guidance(adapter_id.to_owned()).await.map_err(|error| {
        eprintln!("guidance failed: {error:?}");
    })?;
    println!("{}", json!({ "guidance": guidance }));
    Ok(())
}

/// Drive one target `build` over the typed inputs read from `inputs-dir`;
/// print the report as one JSON line and carry its status in the exit.
async fn build(adapter_id: &str, slice: &str, inputs_dir: &str) -> Result<(), ()> {
    let inputs = read_inputs(inputs_dir)?;
    eprintln!("eval: building `{slice}` via `{adapter_id}` with {} inputs", inputs.len());

    let report = target::build(adapter_id.to_owned(), slice.to_owned(), inputs, eval_tree()).await;
    let report = report.map_err(|error| {
        eprintln!("build failed: {error:?}");
    })?;
    conclude(&report)
}

/// Drive one target `merge` over the changeset read from
/// `<inputs-dir>/changeset.json`; print the report as one JSON line and
/// carry its status in the exit.
async fn merge(adapter_id: &str, slice: &str, inputs_dir: &str) -> Result<(), ()> {
    let delta = read_changeset(inputs_dir)?;
    eprintln!("eval: merging `{slice}` via `{adapter_id}` with {} edits", delta.edits.len());

    let report = target::merge(adapter_id.to_owned(), slice.to_owned(), delta, eval_tree()).await;
    let report = report.map_err(|error| {
        eprintln!("merge failed: {error:?}");
    })?;
    conclude(&report)
}

// The scratch project mount every eval operates on.
fn eval_tree() -> WorkingTree {
    WorkingTree {
        base: "eval".to_owned(),
        subpath: None,
    }
}

/// Print the report as one JSON line and map its status onto the exit.
fn conclude(report: &Report) -> Result<(), ()> {
    println!("{}", render(report));
    match report.status {
        Status::Success => Ok(()),
        Status::Failure => Err(()),
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

/// Read and parse `<dir>/lead.json` into the seam `lead` record.
fn read_lead(dir: &str) -> Result<Lead, ()> {
    let value = read_json(&format!("{dir}/lead.json"))?;
    parse_lead(&value).ok_or_else(|| {
        eprintln!(
            "lead.json must mirror the `lead` record: string `lead` and `synopsis`, \
             optional string-array `topics`"
        );
    })
}

fn parse_lead(value: &Value) -> Option<Lead> {
    Some(Lead {
        lead: value.get("lead")?.as_str()?.to_owned(),
        synopsis: value.get("synopsis")?.as_str()?.to_owned(),
        topics: match value.get("topics") {
            None | Some(Value::Null) => Vec::new(),
            Some(topics) => strings(topics)?,
        },
    })
}

/// Read and parse `<dir>/changeset.json` into the seam `changeset` record.
fn read_changeset(dir: &str) -> Result<Changeset, ()> {
    let value = read_json(&format!("{dir}/changeset.json"))?;
    parse_changeset(&value).ok_or_else(|| {
        eprintln!(
            "changeset.json must mirror the `changeset` record: string `base`, `edits` \
             array of {{\"path\", \"content\"?}} objects (string or null `content`)"
        );
    })
}

fn parse_changeset(value: &Value) -> Option<Changeset> {
    let edits = value
        .get("edits")?
        .as_array()?
        .iter()
        .map(|edit| {
            Some(Edit {
                path: edit.get("path")?.as_str()?.to_owned(),
                content: match edit.get("content") {
                    None | Some(Value::Null) => None,
                    Some(content) => Some(content.as_str()?.to_owned()),
                },
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Changeset {
        base: value.get("base")?.as_str()?.to_owned(),
        edits,
    })
}

fn read_json(path: &str) -> Result<Value, ()> {
    let body = std::fs::read_to_string(path).map_err(|error| {
        eprintln!("reading `{path}`: {error}");
    })?;
    serde_json::from_str(&body).map_err(|error| {
        eprintln!("parsing `{path}`: {error}");
    })
}

fn strings(value: &Value) -> Option<Vec<String>> {
    value.as_array()?.iter().map(|entry| entry.as_str().map(str::to_owned)).collect()
}

/// Render the surveyed leads as one JSON line.
fn render_leads(leads: &[Lead]) -> String {
    Value::Array(
        leads
            .iter()
            .map(|lead| {
                json!({
                    "lead": lead.lead,
                    "synopsis": lead.synopsis,
                    "topics": lead.topics,
                })
            })
            .collect(),
    )
    .to_string()
}

// Lowercase-Debug rendering of a WIT enum variant. Sound only while every
// rendered variant is single-word (Debug drops a kebab-case hyphen); a
// multi-word variant needs an explicit match instead of this helper.
fn variant(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}").to_lowercase()
}

/// Render the extracted evidence as one JSON line.
fn render_evidence(evidence: &Evidence) -> String {
    json!({
        "authority": variant(&evidence.authority),
        "claims": evidence.claims.iter().map(|claim| {
            json!({
                "kind": variant(&claim.kind),
                "id": claim.id,
                "path": claim.path,
                "synopsis": claim.synopsis,
                "backing": claim.backing.as_ref().map(|backing| match backing {
                    Backing::Payload(payload) => json!({ "payload": payload }),
                    Backing::Path(path) => json!({ "path": path }),
                }),
            })
        }).collect::<Vec<_>>(),
    })
    .to_string()
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
                "severity": variant(&finding.severity),
                "detail": finding.detail,
            })
        }).collect::<Vec<_>>(),
        "outputs": report.outputs.iter().map(|output| {
            json!({
                "platform": variant(&output.platform),
                "path": output.path,
            })
        }).collect::<Vec<_>>(),
        "ui-surface": report.ui_surface.as_ref().map(|surface| json!({ "screens": surface.screens })),
    })
    .to_string()
}

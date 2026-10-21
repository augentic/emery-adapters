//! `ui-ir-behaviour check <behaviour.json>...` validates against the staged
//! schema; `ui-ir-behaviour coverage <behaviour.json> <computed.json>` lists
//! the link and button elements of a capture (the nodes web-to-ir leaves a
//! `TODO(event)` hole on) and whether a trigger resolves each.

#![expect(clippy::print_stdout, reason = "a command-line tool reports on stdout")]

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use serde_json::{Value, json};

fn read(path: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

fn check(paths: &[String]) -> Result<bool, String> {
    let mut clean = true;
    for path in paths {
        let violations = ui_ir_behaviour::validate(&read(path)?)?;
        println!("{path}: {} violation(s)", violations.len());
        for line in &violations {
            println!("  {line}");
        }
        clean &= violations.is_empty();
    }
    Ok(clean)
}

/// Every node a trigger claims, with whether that trigger has effects.
fn trigger_nodes(doc: &Value) -> BTreeMap<String, bool> {
    let mut nodes = BTreeMap::new();
    for trigger in doc["triggers"].as_array().into_iter().flatten() {
        let on = &trigger["on"];
        let has_effects = trigger["does"].as_array().is_some_and(|d| !d.is_empty());
        let lists = std::iter::once(&on["nodes"])
            .chain(on["submitters"].as_array().into_iter().flatten().map(|s| &s["nodes"]));
        for list in lists {
            for node in list.as_array().into_iter().flatten() {
                if let Some(id) = node.as_str() {
                    let entry = nodes.entry(id.to_string()).or_insert(false);
                    *entry |= has_effects;
                }
            }
        }
    }
    nodes
}

fn coverage(behaviour: &str, computed: &str) -> Result<bool, String> {
    let doc = read(behaviour)?;
    let screen = doc["screen"].as_str().ok_or("behaviour.json has no screen")?.to_string();
    let claimed = trigger_nodes(&doc);
    let elements = read(computed)?;
    let mut rows = Vec::new();
    for element in elements.as_array().into_iter().flatten() {
        let tag = element["tag"].as_str().unwrap_or_default();
        if tag != "a" && tag != "button" {
            continue;
        }
        let path = element["path"].as_str().unwrap_or_default();
        let id = ui_ir_behaviour::node_id(&screen, path);
        let state = match claimed.get(&id) {
            Some(true) => "resolvable",
            Some(false) => "claimed_without_effects",
            None => "unclaimed",
        };
        rows.push(json!({ "node": id, "tag": tag, "path": path, "state": state }));
    }
    let count = |state: &str| rows.iter().filter(|r| r["state"] == json!(state)).count();
    let in_capture: BTreeSet<String> =
        rows.iter().filter_map(|r| r["node"].as_str().map(str::to_string)).collect();
    let outside = claimed.keys().filter(|k| !in_capture.contains(*k)).count();
    let report = json!({
        "screen": screen,
        "event_holes": rows.len(),
        "matched_resolvable": count("resolvable"),
        "matched_without_effects": rows.iter().filter(|r| r["state"] == json!("claimed_without_effects")).collect::<Vec<_>>(),
        "unclaimed": rows.iter().filter(|r| r["state"] == json!("unclaimed")).collect::<Vec<_>>(),
        "trigger_nodes_outside_capture": outside,
    });
    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
    Ok(true)
}

#[expect(clippy::disallowed_methods, reason = "a native operator tool, not a guest")]
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("check") if args.len() > 1 => check(&args[1..]),
        Some("coverage") if args.len() == 3 => coverage(&args[1], &args[2]),
        _ => Err("usage: ui-ir-behaviour check <behaviour.json>... | coverage <behaviour.json> <computed.json>".into()),
    };
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(reason) => {
            println!("error: {reason}");
            ExitCode::from(2)
        }
    }
}

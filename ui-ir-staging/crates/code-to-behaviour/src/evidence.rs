//! A behaviour document as Emery evidence of the `behaviour` source kind.

use emery_sdk::{Claim, ClaimKind, Evidence};
use serde_json::Value;

/// The claims a behaviour document holds for Emery.
///
/// - Every effect that calls a function is a `call` claim at the effect's anchor, its
///   `callee` the function.
/// - Every observation that showed an effect happen is an `example` claim at the anchor of
///   the trigger, guard, or rule it observed, its `replay-digest` the recorded step's digest.
///
/// Example ids are `<screen>.<entry>.<recording>.<step>`, kebab-case. No claim is a
/// `requirement`: a requirement is the model's to state, from these and the code.
#[must_use]
pub fn evidence(doc: &Value) -> Evidence {
    let screen = slug(doc["screen"].as_str().unwrap_or("screen"));
    let mut claims = Vec::new();
    for trigger in items(&doc["triggers"]) {
        calls(&trigger["does"], &mut claims);
        let entry = slug(trigger["id"].as_str().unwrap_or("trigger"));
        let summary = format!(
            "{} on {} {}",
            trigger["on"]["gesture"].as_str().unwrap_or("tap"),
            trigger["on"]["match"]["role"].as_str().unwrap_or("element"),
            trigger["on"]["match"]["text"].as_str().map(|t| format!("\"{t}\"")).unwrap_or_default(),
        );
        examples(&screen, &entry, summary.trim(), trigger, &mut claims);
    }
    for (i, guard) in items(&doc["guards"]).enumerate() {
        let summary = format!(
            "guard on {} goes to {}",
            guard["route"].as_str().unwrap_or("?"),
            guard["go"].as_str().unwrap_or("?")
        );
        examples(&screen, &format!("guard-{i}"), &summary, guard, &mut claims);
    }
    for (i, rule) in items(&doc["validation"]).enumerate() {
        let summary = format!(
            "{} rule on {} shows its message",
            rule["rule"].as_str().unwrap_or("?"),
            rule["field"].as_str().unwrap_or("?")
        );
        examples(&screen, &format!("rule-{i}"), &summary, rule, &mut claims);
    }
    Evidence { claims }
}

fn items(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

fn anchor(entry: &Value) -> Option<&str> {
    entry["evidence"][0]["path"].as_str()
}

fn calls(effects: &Value, out: &mut Vec<Claim>) {
    for effect in items(effects) {
        if effect["kind"] == "call"
            && let Some(callee) = effect["callee"].as_str()
        {
            let mut claim = Claim::new(ClaimKind::Call).with_extra(Claim::CALLEE, callee);
            if let Some(path) = anchor(effect) {
                claim = claim.at(path);
            }
            if let (Some(method), Some(url)) =
                (effect["operation"]["method"].as_str(), effect["operation"]["url"].as_str())
            {
                claim = claim.with_synopsis(format!("{method} {url}"));
            }
            out.push(claim);
        }
        calls(&effect["on_success"], out);
        calls(&effect["on_failure"], out);
    }
}

fn examples(screen: &str, entry: &str, summary: &str, observed: &Value, out: &mut Vec<Claim>) {
    for obs in items(&observed["observed"]) {
        if items(&obs["confirms"]).next().is_none() {
            continue;
        }
        let (Some(recording), Some(step), Some(digest)) =
            (obs["recording"].as_str(), obs["step"].as_str(), obs["replay_digest"].as_str())
        else {
            continue;
        };
        let mut claim = Claim::new(ClaimKind::Example)
            .with_id(format!("{screen}.{entry}.{}.{}", slug(recording), slug(step)))
            .with_synopsis(format!("{summary}, seen in run {recording} at step {step}"))
            .with_extra(Claim::REPLAY_DIGEST, digest);
        if let Some(path) = anchor(observed) {
            claim = claim.at(path);
        }
        out.push(claim);
    }
}

/// Kebab-case: lower-case ASCII letters and digits, runs of anything else one `-`.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

//! A behaviour document as Emery evidence of the `behaviour` source kind: the
//! facts joined without a model turn.

use emery_sdk::serde_json::Value;
use emery_sdk::{Claim, ClaimKind, Evidence};

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
    let screen = screen(doc);
    let mut claims = Vec::new();
    for trigger in items(&doc["triggers"]) {
        calls(&trigger["does"], &mut claims);
        let entry = id(trigger["id"].as_str(), "trigger");
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
            .with_id(format!(
                "{screen}.{entry}.{}.{}",
                id(Some(recording), "recording"),
                id(Some(step), "step")
            ))
            .with_synopsis(format!("{summary}, seen in run {recording} at step {step}"))
            .with_extra(Claim::REPLAY_DIGEST, digest);
        if let Some(path) = anchor(observed) {
            claim = claim.at(path);
        }
        out.push(claim);
    }
}

/// The document's screen as an id: [`emery_sdk::kebab`] of `screen`, or `screen` when that leaves
/// nothing. The seam stem ([`crate::survey`]) and every example id use it, so they agree.
pub fn screen(doc: &Value) -> String {
    id(doc["screen"].as_str(), "screen")
}

/// `text` as [`emery_sdk::kebab`] spells it, or `fallback` when it is absent or leaves nothing.
fn id(text: Option<&str>, fallback: &str) -> String {
    text.and_then(emery_sdk::kebab).unwrap_or_else(|| fallback.to_owned())
}

#[cfg(test)]
mod tests {
    use emery_sdk::serde_json::json;

    use super::*;

    #[test]
    fn observed_login() {
        let digest = format!("sha256:{}", "1".repeat(64));
        let doc = json!({
            "version": 1, "screen": "login", "routes": [], "guards": [], "state": {}, "validation": [], "transitions": [],
            "triggers": [{
                "id": "t-000000000001",
                "on": { "gesture": "submit", "match": { "role": "form", "text": null, "href": null, "name": null, "in_overlay": false }, "nodes": [], "match_status": "unique" },
                "does": [{
                    "kind": "call", "callee": "src/api.ts:login", "args": [],
                    "operation": { "method": "POST", "url": "/api/login" },
                    "on_success": [{ "kind": "call", "callee": "global:localStorage.setItem", "args": [], "evidence": [{ "path": "src/api.ts#L9" }] }],
                    "evidence": [{ "path": "src/api.ts#L4-L6" }]
                }],
                "evidence": [{ "path": "src/routes/(auth)/login.tsx#L20" }],
                "observed": [
                    { "recording": "x-login-success", "step": "submit", "replay_digest": digest, "confirms": [0] },
                    { "recording": "x-login-empty", "step": "submit", "replay_digest": digest, "confirms": [] }
                ]
            }]
        });
        let evidence = evidence(&doc);
        assert_eq!(evidence.findings(), Vec::<String>::new());
        let kinds: Vec<ClaimKind> = evidence.claims.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, [ClaimKind::Call, ClaimKind::Call, ClaimKind::Example]);
        let example = &evidence.claims[2];
        assert_eq!(example.id.as_deref(), Some("login.t-000000000001.x-login-success.submit"));
        assert_eq!(example.path.as_deref(), Some("src/routes/(auth)/login.tsx#L20"));
        assert_eq!(example.extras["replay-digest"], json!(digest));
        assert_eq!(evidence.claims[0].synopsis.as_deref(), Some("POST /api/login"));
    }

    #[test]
    fn ids_fall_back_when_kebab_leaves_nothing() {
        assert_eq!(screen(&json!({ "screen": "OrdersAPI" })), "orders-api");
        assert_eq!(screen(&json!({ "screen": "--" })), "screen");
        assert_eq!(screen(&json!({})), "screen");
        assert_eq!(id(Some("t-000000000001"), "trigger"), "t-000000000001");
        assert_eq!(id(None, "trigger"), "trigger");
    }
}

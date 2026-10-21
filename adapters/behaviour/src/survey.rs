//! Finds the behaviour documents of a workspace, one seam each, and joins
//! their facts to the model's meaning.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use emery_sdk::serde_json::{self, Value, json};
use emery_sdk::workspace::{self, Entry};
use emery_sdk::{Claim, Context, Error, Evidence, Model, Seam, SourceContent, bad_request};
use sha2::{Digest, Sha256};

use crate::PROSE;

/// The file name the fact layer writes.
const DOCUMENT: &str = "behaviour.json";
const NEVER_DIRS: &[&str] = &["node_modules", "vendor", "target", "dist", "build"];
const PRODUCER: &str = "emery-behaviour";

pub struct Survey {
    pub seams: Vec<Seam>,
    facts: Vec<Claim>,
}

pub fn survey<P: Model>(ctx: &Context<'_, P>) -> Result<Survey, Error> {
    let source = &ctx.input.name;
    let SourceContent::Workspace(root) = &ctx.input.content else {
        return Err(bad_request!(
            "`{source}`: a behaviour source is a workspace holding `{DOCUMENT}` files, not a value"
        ));
    };
    let documents = workspace::list(root, keep)?;
    if documents.is_empty() {
        return Err(bad_request!("`{source}` holds no `{DOCUMENT}`"));
    }

    let mut seams = Vec::new();
    let mut facts = Vec::new();
    for path in documents {
        let text = std::fs::read_to_string(Path::new(root).join(&path))
            .map_err(|e| bad_request!("`{source}`: cannot read `{path}`: {e}"))?;
        let doc: Value = serde_json::from_str(&text)
            .map_err(|e| bad_request!("`{source}`: `{path}` is not JSON: {e}"))?;
        seams.push(seam(&path, &doc));
        facts.extend(crate::evidence::evidence(&doc).claims);
    }
    Ok(Survey { seams, facts })
}

/// The seam of one document: its stem is the screen id the example claims carry.
fn seam(path: &str, doc: &Value) -> Seam {
    let screen = crate::evidence::screen(doc);
    let cited = cited(doc);
    let mut files = vec![path.to_owned()];
    files.extend(cited.iter().map(|a| a.split('#').next().unwrap_or(a).to_owned()));
    Seam {
        text: format!(
            "Screen `{screen}` — behaviour document `{path}` — stem `{screen}`.\n\n\
                 The document is the fact layer: every trigger, effect, guard and validation rule \
                 with its `file#L` evidence, and `observed` entries from recorded runs. State what \
                 they are for; point every claim at the evidence the document gives and name the \
                 JSON pointer of the entry it explains in `behaviour-pointer`."
        ),
        files,
        stems: vec![screen],
        anchors: cited.into_iter().collect(),
        ..Seam::default()
    }
}

impl Survey {
    /// The model's claims, each marked inferred with its producer, then the facts.
    pub fn join(self, meaning: Evidence) -> Evidence {
        let producer = json!({ "name": PRODUCER, "prompt-digest": prompt_digest() });
        let mut claims: Vec<Claim> = meaning
            .claims
            .into_iter()
            .filter(|claim| claim.path.is_some())
            .map(|claim| {
                claim.with_extra("status", "inferred").with_extra("producer", producer.clone())
            })
            .collect();
        claims.extend(self.facts);
        Evidence { claims }
    }
}

fn keep(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    match entry {
        Entry::Dir(_) => !NEVER_DIRS.contains(&entry.name()),
        Entry::File(_) => entry.name() == DOCUMENT,
    }
}

/// Every `file#L` the document cites, in order.
fn cited(doc: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    walk(doc, &mut out);
    out
}

fn walk(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::Array(items)) = map.get("evidence") {
                for item in items {
                    if let Some(path) = item["path"].as_str() {
                        out.insert(path.to_owned());
                    }
                }
            }
            map.values().for_each(|x| walk(x, out));
        }
        Value::Array(items) => items.iter().for_each(|x| walk(x, out)),
        _ => {}
    }
}

/// `sha256:` over the embedded prompt, so a claim names the prose it came from.
fn prompt_digest() -> String {
    let mut hash = Sha256::new();
    for doc in PROSE {
        hash.update(doc.path.as_bytes());
        hash.update([0]);
        hash.update(doc.body.as_bytes());
    }
    hash.finalize().iter().fold(String::from("sha256:"), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use emery_sdk::ClaimKind;

    use super::*;

    #[test]
    fn cites_every_evidence_path() {
        let doc = json!({
            "triggers": [{ "evidence": [{ "path": "a.tsx#L1" }],
                           "does": [{ "evidence": [{ "path": "b.ts#L2-L3" }] }] }],
            "validation": [{ "evidence": [{ "path": "a.tsx#L4" }] }]
        });
        let got: Vec<_> = cited(&doc).into_iter().collect();
        assert_eq!(got, ["a.tsx#L1", "a.tsx#L4", "b.ts#L2-L3"]);
    }

    #[test]
    fn a_camel_case_screen_gives_the_seam_stem_the_example_ids_use() {
        let digest = format!("sha256:{}", "1".repeat(64));
        let doc = json!({
            "screen": "OrdersAPI",
            "triggers": [{
                "id": "t-1", "on": { "gesture": "tap" }, "does": [],
                "evidence": [{ "path": "src/orders.tsx#L3" }],
                "observed": [{ "recording": "orders", "step": "open", "replay_digest": digest, "confirms": [0] }]
            }]
        });
        let seam = seam("behaviour.json", &doc);
        assert_eq!(seam.stems, ["orders-api"]);
        let claims = crate::evidence::evidence(&doc).claims;
        let ids: Vec<_> = claims.iter().filter_map(|c| c.id.as_deref()).collect();
        assert_eq!(ids, ["orders-api.t-1.orders.open"]);
        assert!(seam.text.contains("Screen `orders-api`"));
    }

    #[test]
    fn meaning_is_inferred_and_unanchored_is_dropped() {
        let survey = Survey {
            seams: vec![],
            facts: vec![Claim::new(ClaimKind::Call)],
        };
        let meaning = Evidence {
            claims: vec![
                Claim::new(ClaimKind::Requirement).with_id("login.sign-in").at("a.tsx#L1"),
                Claim::new(ClaimKind::Requirement).with_id("login.loose"),
            ],
        };
        let joined = survey.join(meaning);
        assert_eq!(joined.claims.len(), 2);
        assert_eq!(joined.claims[0].extra("status"), Some("inferred"));
        assert!(prompt_digest().starts_with("sha256:"));
    }
}

//! The Emery projection of a behaviour document.

use code_to_behaviour::evidence;
use emery_sdk::ClaimKind;
use serde_json::json;

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

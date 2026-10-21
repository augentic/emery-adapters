//! End-to-end checks over the fixture app in `tests/fixtures/app`.

use std::path::PathBuf;

use code_to_behaviour::{Options, Output, extract};
use serde_json::{Value, json};

fn run(entry: &str, screen: &str) -> Output {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app");
    extract(&Options {
        source,
        entry: entry.to_string(),
        screen: screen.to_string(),
        aliases: vec![("@/".to_string(), "src/".to_string())],
    })
}

fn trigger<'a>(doc: &'a Value, role: &str, text: &str) -> &'a Value {
    doc["triggers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["on"]["match"]["role"] == role && t["on"]["match"]["text"] == text)
        .unwrap_or_else(|| panic!("no {role} trigger with text {text}"))
}

#[test]
fn login_is_valid_and_deterministic() {
    let first = run("src/routes/login.tsx", "login");
    let second = run("src/routes/login.tsx", "login");
    assert_eq!(first.behaviour, second.behaviour);
    assert_eq!(first.unmapped, second.unmapped);
    let violations = ui_ir_behaviour::validate(&first.behaviour).expect("schema compiles");
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn login_submit_chain() {
    let doc = run("src/routes/login.tsx", "login").behaviour;
    let form = &doc["triggers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["on"]["gesture"] == "submit")
        .expect("submit trigger");
    assert_eq!(form["on"]["submitters"][0]["match"]["text"], "Log In");
    assert_eq!(form["guard"][0]["expr"], "loginMutation.isPending");
    assert_eq!(form["does"][0]["kind"], "validate");
    assert_eq!(form["does"][0]["fields"], json!(["username", "password"]));
    let call = &form["does"][1];
    assert_eq!(call["callee"], "src/client/sdk.ts:LoginService.loginAccessToken");
    assert_eq!(
        call["operation"],
        json!({
            "method": "POST", "url": "/api/v1/login/access-token",
            "evidence": [{ "path": "src/client/sdk.ts#L5" }],
        })
    );
    assert_eq!(call["on_success"][0]["callee"], "global:localStorage.setItem");
    assert_eq!(
        call["on_success"][1],
        json!({ "kind": "navigate", "to": "/", "evidence": [{ "path": "src/hooks/useAuth.ts#L22" }] })
    );
    assert_eq!(call["on_failure"][0]["kind"], "show");
    assert_eq!(call["on_failure"][0]["title"], "Login failed");
    assert_eq!(call["on_failure"][0]["tone"], "error");
}

#[test]
fn login_guard_validation_routes() {
    let doc = run("src/routes/login.tsx", "login").behaviour;
    assert_eq!(doc["guards"][0]["route"], "/login");
    assert_eq!(doc["guards"][0]["go"], "/");
    assert_eq!(
        doc["guards"][0]["when"]["x-web"]["storage"][0],
        json!({ "op": "get", "key": "access_token" })
    );
    let rules: Vec<(String, String)> = doc["validation"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            (
                r["field"].as_str().unwrap_or_default().into(),
                r["rule"].as_str().unwrap_or_default().into(),
            )
        })
        .collect();
    // the zod schema's rules, then the input's own `type="email"` (message null)
    assert_eq!(
        rules,
        [
            ("password".into(), "min_length".into()),
            ("username".into(), "email".into()),
            ("username".into(), "email".into())
        ]
    );
    let native = doc["validation"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| r["evidence"][0]["path"] == "src/routes/login.tsx#L36")
        .cloned()
        .unwrap_or_default();
    assert_eq!((native["rule"].as_str(), &native["message"]), (Some("email"), &Value::Null));
    assert_eq!(doc["validation"][0]["value"], 8);
    assert_eq!(doc["validation"][0]["evidence"][0]["path"], "src/routes/login.tsx#L12");
    let paths: Vec<&str> =
        doc["routes"].as_array().into_iter().flatten().filter_map(|r| r["path"].as_str()).collect();
    assert_eq!(paths, ["/", "/login", "/signup"]);
    assert_eq!(trigger(&doc, "link", "Sign up")["does"][0]["to"], "/signup");
}

#[test]
fn navbar_sheet() {
    let out = run("src/App.tsx", "index");
    let doc = &out.behaviour;
    assert_eq!(
        ui_ir_behaviour::validate(doc).expect("schema compiles"),
        [] as [std::string::String; 0]
    );
    let state = &doc["state"]["src/components/Navbar.tsx:Navbar.isOpen"];
    assert_eq!(state["initial"], false);
    assert_eq!(state["controls"]["overlay"], "dialog");
    let menu = trigger(doc, "button", "Menu");
    assert_eq!(menu["does"][0]["kind"], "toggle");
    assert_eq!(menu["does"][0]["state"], "src/components/Navbar.tsx:Navbar.isOpen");
    let features = trigger(doc, "link", "Features");
    assert_eq!(features["on"]["match"]["in_overlay"], true);
    assert_eq!(features["on"]["match"]["href"], "#features");
    let kinds: Vec<&str> = features["does"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["set", "scroll_to"]);
    assert_ne!(features["id"], trigger(doc, "link", "FAQ")["id"]);
    let close = trigger(doc, "button", "Close");
    assert_eq!(close["on"]["match"]["in_overlay"], true);
    let docs = trigger(doc, "link", "Docs");
    assert_eq!(docs["x-web"]["new_tab"], true);
    assert_eq!(docs["does"][0]["kind"], "open_url");
    assert_eq!(out.unmapped["unmapped"][0]["key"], "attr:onOpenChange");
}

#[test]
fn route_groups_component_option_zod_messages_and_constraints() {
    let doc = run("src/routes/(auth)/sign-in.tsx", "sign-in").behaviour;
    let paths: Vec<&str> =
        doc["routes"].as_array().into_iter().flatten().filter_map(|r| r["path"].as_str()).collect();
    assert_eq!(paths, ["/home", "/sign-in"]);
    let rules: Vec<(&str, &str, &Value)> = doc["validation"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            (
                r["field"].as_str().unwrap_or_default(),
                r["rule"].as_str().unwrap_or_default(),
                &r["message"],
            )
        })
        .collect();
    assert_eq!(
        rules,
        [
            ("code", "min_length", &json!("Six digits")),
            ("email", "email", &json!("Not an email")),
            ("email", "min_length", &json!("Please enter your email.")),
            ("nickname", "min_length", &Value::Null),
            ("nickname", "required", &Value::Null),
        ]
    );
    let submit = doc["triggers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["on"]["gesture"] == "submit")
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        submit["does"][1],
        json!({ "kind": "navigate", "to": "/home", "evidence": [{ "path": "src/features/sign-in.tsx#L19" }] })
    );
    let toggle = doc["triggers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["on"]["gesture"] == "tap")
        .cloned()
        .unwrap_or_default();
    assert_eq!(toggle["on"]["match"]["text"], Value::Null);
    assert_eq!(toggle["on"]["match"]["text_in"], json!(["Hide", "Show"]));
}

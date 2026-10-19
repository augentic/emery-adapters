//! Renders the files a scaffold writes and places the lines it adds to the lists.

use std::fmt::Write as _;

use super::names::{escape, flat};
use super::{Act, Consumer, Function, Handler, Stem, Test, Type, route};

// The template's `rustfmt.toml` width, so a scaffolded line the formatter
// would break is written broken.
const MAX_WIDTH: usize = 100;
const PATH_LIST_ANCHOR: &str = "common = { path = \"crates/common\" }";
const ROUTES_ANCHOR: &str = "// one entry per part with routes, each added at the end";
const TOPICS_ANCHOR: &str = "// one entry per part with topics, each added at the end";
const DERIVES: &str = "Debug, Clone, PartialEq, Eq, Serialize, Deserialize";

// The stem's manifest, as the layout reference spells it.
const MANIFEST: &str = r#"[package]
name = "<package>"
description = "The `<stem>` stem"
publish.workspace = true
edition.workspace = true
rust-version.workspace = true
version.workspace = true

[lints]
workspace = true

# Target-gated so the guest's wasm32 dependency graph never sees the doubles.
[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
tokio.workspace = true
tower.workspace = true

# One line per crate the stem's files use, each added at the end.
[dependencies]
axum.workspace = true
common.workspace = true
omnia-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
"#;

// The stem's entry, as the layout reference spells it, with one part's
// lines in its lists.
const ENTRY: &str = r"//! # <stem>
//!
//! The `<stem>` stem.

pub mod <part>;

use common::Capabilities;
use omnia_sdk::api::{Client, messaging};

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
<routes>    ])
}

/// The stem's topics, registered on the root messaging router under `env`.
#[must_use]
pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {
    common::topics(router, env).parts([
        // one entry per part with topics, each added at the end
<topics>    ])
}
";

// The decoder a pre-rendered route function reads the input with: what the
// SDK's `post` does, spelled in the part so the encoder beside it is the one
// place a status changes.
const DECODE_JSON: &str = r#"// The JSON body with the path parameters inserted as string fields, as the
// SDK's `post` decodes an input.
fn decode_json<I: DeserializeOwned>(
    params: &[(String, String)], body: &[u8],
) -> Result<I, DecodeError> {
    let mut value = if body.is_empty() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_slice(body).map_err(|error| DecodeError::new(error.to_string()))?
    };
    let serde_json::Value::Object(object) = &mut value else {
        return Err(DecodeError::new("the request body must be a JSON object"));
    };
    for (key, param) in params {
        object.insert(key.clone(), serde_json::Value::String(param.clone()));
    }
    serde_json::from_value(value).map_err(|error| DecodeError::new(error.to_string()))
}
"#;

// Where a list takes a line.
pub enum Place {
    // The root manifest's path list: after `common`'s line and the stems'
    // after it.
    PathList,
    // The root manifest's crate list, and the topic table: the end of the
    // file.
    FileEnd,
    // Before the line that starts with the anchor, as the root route and
    // topic lists take a statement.
    Before(&'static str),
    // Among the `pub mod` lines of a stem's entry, in the order the
    // formatter keeps them.
    PubModList,
    // At the end of the routes array of a stem's entry.
    RoutesArray,
    // At the end of the topics array of a stem's entry.
    TopicsArray,
}

// `text` with `line` where `place` says, or none where the text holds no
// anchor for it.
pub fn insert(text: &str, line: &str, place: &Place) -> Option<String> {
    let mut lines: Vec<&str> = text.lines().collect();
    let at = match place {
        Place::PathList => {
            let mut at = lines.iter().position(|held| *held == PATH_LIST_ANCHOR)? + 1;
            while lines.get(at).is_some_and(|held| held.contains(" = { path = \"crates/")) {
                at += 1;
            }
            at
        }
        Place::FileEnd => lines.len(),
        Place::Before(anchor) => {
            lines.iter().position(|held| held.trim_start().starts_with(anchor))?
        }
        Place::PubModList => {
            let first = lines.iter().position(|held| held.trim_start().starts_with("pub mod "))?;
            let last = lines.iter().rposition(|held| held.trim_start().starts_with("pub mod "))?;
            lines[first..=last]
                .iter()
                .position(|held| held.trim_start().starts_with("pub mod ") && *held > line)
                .map_or(last + 1, |offset| first + offset)
        }
        Place::RoutesArray => array_end(&lines, ROUTES_ANCHOR)?,
        Place::TopicsArray => array_end(&lines, TOPICS_ANCHOR)?,
    };
    lines.insert(at, line);
    Some(lines.join("\n") + "\n")
}

// The index of the `])` closing the array `anchor` opens.
fn array_end(lines: &[&str], anchor: &str) -> Option<usize> {
    let opened = lines.iter().position(|held| held.trim() == anchor)? + 1;
    Some(opened + lines[opened..].iter().position(|held| held.trim() == "])")?)
}

pub fn manifest(stem: &Stem) -> String {
    MANIFEST.replace("<package>", &stem.package).replace("<stem>", &stem.name)
}

pub fn entry(stem: &Stem) -> String {
    let routes = if stem.handlers.is_empty() {
        String::new()
    } else {
        format!("        {}::routes(),\n", stem.part)
    };
    let topics = if stem.consumers.is_empty() {
        String::new()
    } else {
        format!("        {}::topics,\n", stem.part)
    };
    ENTRY
        .replace("<stem>", &stem.name)
        .replace("<part>", &stem.part)
        .replace("<routes>", &routes)
        .replace("<topics>", &topics)
}

pub fn module(stem: &Stem) -> String {
    let mut text = String::new();
    for line in &stem.doc {
        if line.is_empty() {
            text.push_str("//!\n");
        } else {
            let _ = writeln!(text, "//! {line}");
        }
    }
    let imports = imports(stem);
    if !imports.is_empty() {
        text.push('\n');
        text.push_str(&imports);
    }
    if !stem.handlers.is_empty() {
        text.push_str("\n// The paths the part serves.\n");
        for (constant, path) in &stem.paths {
            let _ = writeln!(text, "pub(crate) const {constant}: &str = \"{}\";", escape(path));
        }
        text.push('\n');
        text.push_str(&routes(stem));
    }
    if !stem.consumers.is_empty() {
        text.push('\n');
        text.push_str(&topics(stem));
    }
    for design_type in &stem.types {
        text.push('\n');
        text.push_str(&design(design_type));
    }
    for handler in &stem.handlers {
        text.push('\n');
        text.push_str(&self::handler(handler));
    }
    for consumer in &stem.consumers {
        text.push('\n');
        text.push_str(&self::consumer(consumer));
    }
    for function in &stem.functions {
        text.push('\n');
        text.push_str(&self::function(function));
    }
    if stem.handlers.iter().any(|handler| handler.route_fn.is_some()) {
        text.push('\n');
        text.push_str(DECODE_JSON);
    }
    text
}

// The test file: the prelude the scenarios act through, each helper only
// where a test calls it, then one ignored test per scenario.
pub fn tests(stem: &Stem) -> String {
    let mut text = format!(
        "//! The acceptance scenarios of the `{}` part of the `{}` stem, over the guest doubles.\n",
        stem.part, stem.name
    );
    let sending = stem.scenarios.iter().any(|test| matches!(test.act, Act::Route(_)));
    let delivering = stem.scenarios.iter().any(|test| matches!(test.act, Act::Topic(_)));
    let with_body = stem
        .scenarios
        .iter()
        .any(|test| matches!(&test.act, Act::Route(route) if carries_body(&route.method)));
    let ident = stem.package.replace('-', "_");

    text.push('\n');
    if sending {
        text.push_str("use axum::body::{Body, to_bytes};\n");
        if with_body {
            text.push_str("use axum::http::header::CONTENT_TYPE;\n");
        }
        text.push_str("use axum::http::{Request, StatusCode};\n");
    }
    let common = if delivering { "{config, topic}" } else { "config" };
    let _ = writeln!(text, "use common::{common};");
    if sending || delivering {
        text.push_str("use omnia_sdk::api::Client;\n");
    }
    if delivering {
        text.push_str("use omnia_sdk::api::messaging::{Delivery, DeliveryError, Router};\n");
    }
    text.push_str("use omnia_test::guest::{MapConfig, Provider};\n");
    if sending {
        text.push_str("use tower::ServiceExt as _;\n");
    }

    text.push_str(
        "\n// The doubles every scenario starts from: set a setting or a response on them in the \
         arrangement.\nfn provider() -> Provider {\n    \
         Provider::default().config(MapConfig::default().with([(config::ENV, \"test\")]))\n}\n",
    );
    if sending {
        let _ = write!(
            text,
            "\n// One request through the stem's router, answered as its status and body.\n\
             async fn send(provider: Provider, request: Request<Body>) -> (StatusCode, Vec<u8>) \
             {{\n    let router = {ident}::router().with_state(Client::new(\"test\", provider));\n    \
             let response = router.oneshot(request).await.expect(\"response\");\n    let status = \
             response.status();\n    let body = to_bytes(response.into_body(), \
             usize::MAX).await.expect(\"body\");\n    (status, body.to_vec())\n}}\n"
        );
    }
    if delivering {
        let _ = write!(
            text,
            "\n// One delivery through the stem's topics, under the test environment.\n\
             async fn deliver(provider: Provider, topic: &str, payload: &[u8]) -> Result<(), \
             DeliveryError> {{\n    let router = {ident}::topics(Router::new(Client::new(\"test\", \
             provider)), \"test\");\n    let delivery = Delivery {{\n        topic: \
             Some(config::topic_for(\"test\", topic)),\n        payload: payload.to_vec(),\n        \
             ..Delivery::default()\n    }};\n    router.handle(delivery).await\n}}\n"
        );
    }

    for test in &stem.scenarios {
        text.push('\n');
        let _ = writeln!(text, "// {} {}: {}", test.id, test.subject, flat(&test.scenario.name));
        for given in &test.scenario.given {
            let _ = writeln!(text, "//   Given {}", flat(given));
        }
        let _ = writeln!(text, "//   When {}", flat(&test.scenario.when));
        let _ = writeln!(text, "//   Then {}", flat(&test.scenario.then));
        for and in &test.scenario.and {
            let _ = writeln!(text, "//   And {}", flat(and));
        }
        text.push_str("#[tokio::test]\n");
        text.push_str(&test_block(stem, test));
        text.push('\n');
    }
    text
}

// A scaffolded test from its `#[ignore]` line to its closing brace: the one
// block the turn replaces, so the brief quotes it as the `find`.
pub fn test_block(stem: &Stem, test: &Test) -> String {
    format!(
        "#[ignore = \"{}\"]\nasync fn {}() {{\n{}    todo!(\"{}\")\n}}",
        escape(&test.ignore),
        test.name,
        act(stem, test),
        escape(&test.marker)
    )
}

// The act of a scenario, laid from the typed route or topic its requirement
// carries: the request sent, the delivery made, or the provider alone.
fn act(stem: &Stem, test: &Test) -> String {
    match &test.act {
        Act::Route(route) => {
            let mut out = String::new();
            let mut format = String::new();
            let mut params = Vec::new();
            for segment in route.path.split('/') {
                if let Some(name) =
                    segment.strip_prefix('{').and_then(|rest| rest.strip_suffix('}'))
                {
                    let wire = name.trim_start_matches('*');
                    let field = super::names::snake(wire);
                    let _ = write!(format, "{{{field}}}");
                    params.push((wire.to_owned(), field));
                } else {
                    format.push_str(segment);
                }
                format.push('/');
            }
            format.pop();
            for (wire, field) in &params {
                let _ = writeln!(out, "    let {field} = \"<{}>\";", escape(wire));
            }
            let target = if params.is_empty() {
                format!("\"{}\"", escape(&route.path))
            } else {
                format!("format!(\"{}\")", escape(&format))
            };
            let builder = format!("Request::{}({target})", route::function(&route.method));
            let calls: Vec<String> = if carries_body(&route.method) {
                vec![
                    ".header(CONTENT_TYPE, \"application/json\")".to_owned(),
                    ".body(Body::from(\"{}\"))".to_owned(),
                    ".expect(\"request\")".to_owned(),
                ]
            } else {
                vec![".body(Body::empty())".to_owned(), ".expect(\"request\")".to_owned()]
            };
            let joined = format!("    let request = {builder}{};", calls.concat());
            if joined.len() <= MAX_WIDTH {
                out.push_str(&joined);
                out.push('\n');
            } else {
                let _ = writeln!(out, "    let request = {builder}");
                for call in &calls {
                    let _ = writeln!(out, "        {call}");
                }
                out.pop();
                out.push_str(";\n");
            }
            out.push_str(
                "    let (status, body) = send(provider(), request).await;\n    let _ = (&status, \
                 &body);\n",
            );
            out
        }
        Act::Topic(topic) => {
            let constant = stem
                .consumers
                .iter()
                .find(|consumer| consumer.topic == *topic)
                .map_or("TOPIC", |consumer| consumer.constant.as_str());
            format!(
                "    let outcome = deliver(provider(), topic::{constant}, b\"{{}}\").await;\n    \
                 let _ = &outcome;\n"
            )
        }
        Act::Call => "    let provider = provider();\n    let _ = &provider;\n".to_owned(),
    }
}

// The body of a handler or a consumer as scaffolded: the borrow that keeps
// the lints quiet and the marker, the one block the turn replaces.
pub fn stub(marker: &str) -> String {
    format!("    let _ = (&input, &context);\n    todo!(\"{}\")", escape(marker))
}

// The body of a function as scaffolded: its marker alone.
pub fn marker_line(marker: &str) -> String {
    format!("    todo!(\"{}\")", escape(marker))
}

// `text` in a fence longer than any run of backticks it holds.
pub fn fenced(lang: &str, text: &str) -> String {
    let longest = text.lines().map(backticks).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}{lang}\n{}\n{fence}\n", text.trim_end_matches('\n'))
}

// Whether the method decodes its input from the body, as the SDK's `post`,
// `put`, and `patch` do.
pub fn carries_body(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH")
}

fn imports(stem: &Stem) -> String {
    let routed = !stem.handlers.is_empty();
    let consuming = !stem.consumers.is_empty();
    let encoding = stem.handlers.iter().any(|handler| handler.route_fn.is_some());
    let mut text = String::new();
    if encoding {
        text.push_str(
            "use axum::Json;\nuse axum::http::StatusCode;\nuse axum::response::IntoResponse;\nuse \
             axum::routing::MethodRouter;\n",
        );
    }
    if routed || consuming {
        let common = if consuming { "{Capabilities, config, topic}" } else { "Capabilities" };
        let _ = writeln!(text, "use common::{common};");
    }
    if routed {
        let mut names: Vec<&str> = stem
            .handlers
            .iter()
            .filter(|handler| handler.route_fn.is_none())
            .map(|handler| route::function(&handler.route.method))
            .collect();
        if encoding {
            names.extend(["DecodeError", "MethodFilter", "RawRequest", "handle_with"]);
        }
        names.sort_unstable();
        names.dedup();
        let names = if let [name] = names.as_slice() {
            (*name).to_owned()
        } else {
            format!("{{{}}}", names.join(", "))
        };
        let _ = writeln!(text, "use omnia_sdk::api::http::{names};");
    }
    if routed || consuming {
        let mut api = Vec::new();
        if routed {
            api.push("Client");
        }
        api.push("Context");
        if consuming {
            api.push("messaging");
        }
        let _ = writeln!(text, "use omnia_sdk::api::{{{}}};", api.join(", "));
    }
    if encoding {
        text.push_str("use serde::de::DeserializeOwned;\n");
    }
    let serializing = routed || !stem.types.is_empty();
    if serializing {
        text.push_str("use serde::{Deserialize, Serialize};\n");
    } else if consuming {
        text.push_str("use serde::Deserialize;\n");
    }
    text
}

// The part's `topics(..)`: one `.route(..)` per consumer, chained over the
// router, each broken as the formatter would.
fn topics(stem: &Stem) -> String {
    let head = "pub(crate) fn topics<P: Capabilities>(\n    router: messaging::Router<P>, env: \
                &str,\n) -> messaging::Router<P> {\n";
    let routes: Vec<(String, String)> = stem
        .consumers
        .iter()
        .map(|consumer| {
            (
                format!("config::topic_for(env, topic::{})", consumer.constant),
                format!("messaging::consume({})", consumer.name),
            )
        })
        .collect();
    let body = if let [(topic, consume)] = routes.as_slice() {
        let joined = format!("    router.route({topic}, {consume})");
        if joined.len() <= MAX_WIDTH {
            joined
        } else {
            format!("    router.route(\n        {topic},\n        {consume},\n    )")
        }
    } else {
        let chained: Vec<String> = routes
            .iter()
            .map(|(topic, consume)| {
                let joined = format!("        .route({topic}, {consume})");
                if joined.len() <= MAX_WIDTH {
                    joined
                } else {
                    format!(
                        "        .route(\n            {topic},\n            {consume},\n        )"
                    )
                }
            })
            .collect();
        format!("    router\n{}", chained.join("\n"))
    };
    format!("{head}{body}\n}}\n")
}

fn routes(stem: &Stem) -> String {
    let routes: Vec<String> = stem.handlers.iter().map(Handler::registration).collect();
    let joined = format!("    axum::Router::new(){}", routes.concat());
    let body = if joined.len() <= MAX_WIDTH {
        joined
    } else {
        let chained: Vec<String> = routes.iter().map(|route| format!("        {route}")).collect();
        format!("    axum::Router::new()\n{}", chained.join("\n"))
    };
    format!("pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {{\n{body}\n}}\n")
}

fn design(design_type: &Type) -> String {
    let longest = design_type.signature.lines().map(backticks).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    let mut text = format!("/// The design type `{}`.\n///\n/// {fence}text\n", design_type.key);
    for line in design_type.signature.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            text.push_str("///\n");
        } else {
            let _ = writeln!(text, "/// {line}");
        }
    }
    let _ =
        writeln!(text, "/// {fence}\n#[derive({DERIVES})]\npub {} {{}}", declaration(design_type));
    text
}

// `struct Order` or `enum Status`, as the type's marker spells it.
pub fn declaration(design_type: &Type) -> String {
    let keyword = if design_type.is_enum { "enum" } else { "struct" };
    format!("{keyword} {}", design_type.name)
}

fn backticks(line: &str) -> usize {
    line.split(|ch| ch != '`').map(str::len).max().unwrap_or(0)
}

fn handler(handler: &Handler) -> String {
    let route = escape(&format!("{} {}", handler.route.method, handler.route.path));
    let mut text = format!("/// The input of `{route}`.\n#[derive(Debug, Deserialize)]\n");
    if handler.params.is_empty() {
        let _ = writeln!(text, "pub struct {} {{}}\n", handler.input);
    } else {
        let _ = writeln!(text, "pub struct {} {{", handler.input);
        for param in &handler.params {
            let wire = escape(&param.wire);
            let _ = writeln!(text, "    /// The `{wire}` path parameter.");
            if param.field != param.wire {
                let _ = writeln!(text, "    #[serde(rename = \"{wire}\")]");
            }
            let _ = writeln!(text, "    pub {}: String,", param.field);
        }
        text.push_str("}\n\n");
    }
    let _ = writeln!(
        text,
        "/// The output of `{route}`.\n#[derive(Debug, Serialize)]\npub struct {} {{}}\n",
        handler.output
    );
    if let Some(route_fn) = &handler.route_fn {
        let _ = write!(
            text,
            "// The `{route}` route: the handler's `Ok` answered as JSON under the status named \
             here.\npub(crate) fn {route_fn}<P: Capabilities>() -> MethodRouter<Client<P>> {{\n    \
             handle_with(\n        MethodFilter::{},\n        {},\n        |raw: RawRequest<'_>| \
             -> Result<{}, DecodeError> {{\n            decode_json(raw.path_params, \
             raw.body)\n        }},\n        |output: {}| (StatusCode::OK, \
             Json(output)).into_response(),\n    )\n}}\n\n",
            handler.route.method, handler.name, handler.input, handler.output
        );
    }
    for stated in &handler.stated {
        let _ = writeln!(text, "// {} {}: {}", stated.id, stated.subject, stated.statement);
    }
    text.push_str(&head(&handler.name, &handler.input, &handler.output));
    let _ = writeln!(text, "{}\n}}", stub(&handler.marker));
    text
}

// A consumer: its input over the delivery, and the handler under its
// requirements, its body the marker.
fn consumer(consumer: &Consumer) -> String {
    let mut text = format!(
        "/// A delivery of `{}`.\n#[derive(Debug, Deserialize)]\npub struct {} {{}}\n\n",
        escape(&consumer.topic),
        consumer.input
    );
    for stated in &consumer.stated {
        let _ = writeln!(text, "// {} {}: {}", stated.id, stated.subject, stated.statement);
    }
    text.push_str(&head(&consumer.name, &consumer.input, "()"));
    let _ = writeln!(text, "{}\n}}", stub(&consumer.marker));
    text
}

// A handler's head, broken as the formatter would.
fn head(name: &str, input: &str, output: &str) -> String {
    let head = format!("pub(crate) async fn {name}<P: Capabilities>(");
    let params = format!("input: {input}, context: Context<P>");
    let tail = format!(") -> omnia_sdk::Result<{output}> {{");
    let joined = format!("{head}{params}{tail}");
    if joined.len() <= MAX_WIDTH {
        return joined + "\n";
    }
    let packed = format!("    {params},");
    if packed.len() <= MAX_WIDTH {
        return format!("{head}\n{packed}\n{tail}\n");
    }
    format!("{head}\n    input: {input},\n    context: Context<P>,\n{tail}\n")
}

fn function(function: &Function) -> String {
    format!(
        "// {} {}: {}\n/// The behaviour `{}` states.\n///\n/// # Errors\n///\n/// As the \
         requirement states.\npub fn {}() -> omnia_sdk::Result<()> {{\n{}\n}}\n",
        function.stated.id,
        function.stated.subject,
        function.stated.statement,
        function.stated.subject,
        function.name,
        marker_line(&function.marker)
    )
}

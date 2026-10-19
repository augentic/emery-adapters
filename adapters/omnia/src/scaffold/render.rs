//! Renders the files a scaffold writes and places the lines it adds to the lists.

mod item;

use std::fmt::Write as _;

pub use self::item::{Call, Expr};
use self::item::{
    Chain, Const, Consts, Doc, Enum, Fence, Field, File, Fn, Item, Struct, TAB, Uses, Vis, closure,
};
use super::names::{escape, flat, snake};
use super::{Act, Consumer, Function, Handler, Stem, Test, Type, route};

const PATH_LIST_ANCHOR: &str = "common = { path = \"crates/common\" }";
const ROUTES_ANCHOR: &str = "// one entry per part with routes, each added at the end";
const TOPICS_ANCHOR: &str = "// one entry per part with topics, each added at the end";
const DERIVES: &str = "Debug, Clone, PartialEq, Eq, Serialize, Deserialize";
const GENERICS: &str = "<P: Capabilities>";

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

// The body of the decoder a pre-rendered route function reads the input
// with: what the SDK's `post` does, spelled in the part so the encoder
// beside it is the one place a status changes.
const DECODE_BODY: &str = r#"    let mut value = if body.is_empty() {
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
    serde_json::from_value(value).map_err(|error| DecodeError::new(error.to_string()))"#;

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

// The topic table's constant for `topic`, under its doc line, as the lines
// added at the end of the table.
pub fn topic_constant(constant: &str, topic: &str) -> String {
    let topic = escape(topic);
    let mut file = File::default();
    file.push(Consts {
        comments: Vec::new(),
        consts: vec![Const {
            docs: Doc::new([format!("The `{topic}` topic.")]),
            vis: Vis::Pub,
            name: constant.to_owned(),
            ty: "&str".to_owned(),
            value: format!("\"{topic}\""),
        }],
    });
    file.render().trim_end().to_owned()
}

// The stem's entry, as the layout reference spells it, with one part's
// lines in its lists.
pub fn entry(stem: &Stem) -> String {
    let mut file = File::default();
    file.push(Item::ModuleDoc(Doc::new([
        format!("# {}", stem.name),
        String::new(),
        format!("The `{}` stem.", stem.name),
    ])));
    file.push(Item::Mods(vec![stem.part.clone()]));
    let mut uses = Uses::default();
    uses.add("common", &["Capabilities"]);
    uses.add("omnia_sdk::api", &["Client", "messaging"]);
    file.push(uses);
    let routes = (!stem.handlers.is_empty()).then(|| format!("{}::routes(),", stem.part));
    file.push(Fn {
        docs: Doc::new(["The stem's routes, merged onto the root router."]),
        vis: Vis::Pub,
        name: "router".to_owned(),
        generics: GENERICS.to_owned(),
        ret: "axum::Router<Client<P>>".to_owned(),
        body: fold("common::routers([", ROUTES_ANCHOR, routes),
        ..Fn::default()
    });
    let topics = (!stem.consumers.is_empty()).then(|| format!("{}::topics,", stem.part));
    file.push(Fn {
        docs: Doc::new(["The stem's topics, registered on the root messaging router under `env`."]),
        attrs: vec!["#[must_use]".to_owned()],
        vis: Vis::Pub,
        name: "topics".to_owned(),
        generics: GENERICS.to_owned(),
        params: vec!["router: messaging::Router<P>".to_owned(), "env: &str".to_owned()],
        ret: "messaging::Router<P>".to_owned(),
        body: fold("common::topics(router, env).parts([", TOPICS_ANCHOR, topics),
        ..Fn::default()
    });
    file.render()
}

// The body of an entry's fold: its array opened under the anchor the
// scaffold's lines and the engine's union merge read, the part's entry
// beneath where the part has one.
fn fold(opener: &str, anchor: &str, entry: Option<String>) -> Vec<String> {
    let mut lines = vec![format!("    {opener}"), format!("        {anchor}")];
    lines.extend(entry.map(|entry| format!("        {entry}")));
    lines.push("    ])".to_owned());
    lines
}

pub fn module(stem: &Stem) -> String {
    let mut file = File::default();
    file.push(Item::ModuleDoc(Doc::new(stem.doc.iter().cloned())));
    let uses = imports(stem);
    if !uses.is_empty() {
        file.push(uses);
    }
    if !stem.handlers.is_empty() {
        file.push(Consts {
            comments: vec!["The paths the part serves.".to_owned()],
            consts: stem
                .paths
                .iter()
                .map(|(constant, path)| Const {
                    docs: Doc::default(),
                    vis: Vis::Crate,
                    name: constant.clone(),
                    ty: "&str".to_owned(),
                    value: format!("\"{}\"", escape(path)),
                })
                .collect(),
        });
        file.push(routes(stem));
    }
    if !stem.consumers.is_empty() {
        file.push(topics(stem));
    }
    for design_type in &stem.types {
        file.push(design(design_type));
    }
    for handler in &stem.handlers {
        self::handler(handler, &mut file);
    }
    for consumer in &stem.consumers {
        self::consumer(consumer, &mut file);
    }
    for function in &stem.functions {
        file.push(self::function(function));
    }
    if stem.handlers.iter().any(|handler| handler.route_fn.is_some()) {
        file.push(decode_json());
    }
    file.render()
}

// The test file: the prelude the scenarios act through, each helper only
// where a test calls it, then one ignored test per scenario.
pub fn tests(stem: &Stem) -> String {
    let sending = stem.scenarios.iter().any(|test| matches!(test.act, Act::Route(_)));
    let delivering = stem.scenarios.iter().any(|test| matches!(test.act, Act::Topic(_)));
    let with_body = stem
        .scenarios
        .iter()
        .any(|test| matches!(&test.act, Act::Route(route) if carries_body(&route.method)));
    let ident = stem.package.replace('-', "_");

    let mut file = File::default();
    file.push(Item::ModuleDoc(Doc::new([format!(
        "The acceptance scenarios of the `{}` part of the `{}` stem, over the guest doubles.",
        stem.part, stem.name
    )])));
    let mut uses = Uses::default();
    if sending {
        uses.add("axum::body", &["Body", "to_bytes"]);
        if with_body {
            uses.add("axum::http::header", &["CONTENT_TYPE"]);
        }
        uses.add("axum::http", &["Request", "StatusCode"]);
    }
    let common: &[&str] = if delivering { &["config", "topic"] } else { &["config"] };
    uses.add("common", common);
    if sending || delivering {
        uses.add("omnia_sdk::api", &["Client"]);
    }
    if delivering {
        uses.add("omnia_sdk::api::messaging", &["Delivery", "DeliveryError", "Router"]);
    }
    uses.add("omnia_test::guest", &["MapConfig", "Provider"]);
    if sending {
        uses.add("tower", &["ServiceExt as _"]);
    }
    file.push(uses);

    file.push(provider());
    if sending {
        file.push(send(&ident));
    }
    if delivering {
        file.push(deliver(&ident));
    }
    for test in &stem.scenarios {
        let mut item = test_fn(stem, test);
        item.comments.push(format!("{} {}: {}", test.id, test.subject, flat(&test.scenario.name)));
        for given in &test.scenario.given {
            item.comments.push(format!("  Given {}", flat(given)));
        }
        item.comments.push(format!("  When {}", flat(&test.scenario.when)));
        item.comments.push(format!("  Then {}", flat(&test.scenario.then)));
        for and in &test.scenario.and {
            item.comments.push(format!("  And {}", flat(and)));
        }
        item.attrs.insert(0, "#[tokio::test]".to_owned());
        file.push(item);
    }
    file.render()
}

// The doubles every scenario starts from.
fn provider() -> Fn {
    Fn {
        comments: vec![
            "The doubles every scenario starts from: set a setting or a response on them in the \
             arrangement."
                .to_owned(),
        ],
        name: "provider".to_owned(),
        ret: "Provider".to_owned(),
        body: vec![
            "    Provider::default().config(MapConfig::default().with([(config::ENV, \"test\")]))"
                .to_owned(),
        ],
        ..Fn::default()
    }
}

// One request through the router of the stem `ident` names.
fn send(ident: &str) -> Fn {
    Fn {
        comments: vec![
            "One request through the stem's router, answered as its status and body.".to_owned(),
        ],
        is_async: true,
        name: "send".to_owned(),
        params: vec!["provider: Provider".to_owned(), "request: Request<Body>".to_owned()],
        ret: "(StatusCode, Vec<u8>)".to_owned(),
        body: vec![
            format!(
                "    let router = {ident}::router().with_state(Client::new(\"test\", provider));"
            ),
            "    let response = router.oneshot(request).await.expect(\"response\");".to_owned(),
            "    let status = response.status();".to_owned(),
            "    let body = to_bytes(response.into_body(), usize::MAX).await.expect(\"body\");"
                .to_owned(),
            "    (status, body.to_vec())".to_owned(),
        ],
        ..Fn::default()
    }
}

// One delivery through the topics of the stem `ident` names.
fn deliver(ident: &str) -> Fn {
    Fn {
        comments: vec![
            "One delivery through the stem's topics, under the test environment.".to_owned(),
        ],
        is_async: true,
        name: "deliver".to_owned(),
        params: vec![
            "provider: Provider".to_owned(),
            "topic: &str".to_owned(),
            "payload: &[u8]".to_owned(),
        ],
        ret: "Result<(), DeliveryError>".to_owned(),
        body: vec![
            format!(
                "    let router = {ident}::topics(Router::new(Client::new(\"test\", provider)), \
                 \"test\");"
            ),
            "    let delivery = Delivery {".to_owned(),
            "        topic: Some(config::topic_for(\"test\", topic)),".to_owned(),
            "        payload: payload.to_vec(),".to_owned(),
            "        ..Delivery::default()".to_owned(),
            "    };".to_owned(),
            "    router.handle(delivery).await".to_owned(),
        ],
        ..Fn::default()
    }
}

// A scaffolded test from its `#[ignore]` line to its closing brace: the one
// block the turn replaces, so the brief quotes it as the `find`.
pub fn test_block(stem: &Stem, test: &Test) -> String {
    let mut file = File::default();
    file.push(test_fn(stem, test));
    file.render().trim_end().to_owned()
}

// A scenario's test as the turn finds it: ignored, its act laid from the
// typed route or topic its requirement carries, its marker last.
fn test_fn(stem: &Stem, test: &Test) -> Fn {
    let mut body = act(stem, test);
    body.push(marker_line(&test.marker));
    Fn {
        attrs: vec![format!("#[ignore = \"{}\"]", escape(&test.ignore))],
        is_async: true,
        name: test.name.clone(),
        body,
        ..Fn::default()
    }
}

// The act of a scenario, laid from the typed route or topic its requirement
// carries: the request sent, the delivery made, or the provider alone.
fn act(stem: &Stem, test: &Test) -> Vec<String> {
    match &test.act {
        Act::Route(route) => {
            let mut lines = Vec::new();
            let mut format = String::new();
            let mut params = Vec::new();
            for segment in route.path.split('/') {
                if let Some(name) =
                    segment.strip_prefix('{').and_then(|rest| rest.strip_suffix('}'))
                {
                    let wire = name.trim_start_matches('*');
                    let field = snake(wire);
                    let _ = write!(format, "{{{field}}}");
                    params.push((wire.to_owned(), field));
                } else {
                    format.push_str(segment);
                }
                format.push('/');
            }
            format.pop();
            for (wire, field) in &params {
                lines.push(format!("    let {field} = \"<{}>\";", escape(wire)));
            }
            let target = if params.is_empty() {
                Expr::from(format!("\"{}\"", escape(&route.path)))
            } else {
                Expr::Macro(Call::new("format!", [format!("\"{}\"", escape(&format))]))
            };
            let mut calls = Vec::new();
            if carries_body(&route.method) {
                calls.push(Call::new(
                    ".header",
                    vec!["CONTENT_TYPE".to_owned(), "\"application/json\"".to_owned()],
                ));
                calls.push(Call::new(".body", vec!["Body::from(\"{}\")".to_owned()]));
            } else {
                calls.push(Call::new(".body", vec!["Body::empty()".to_owned()]));
            }
            calls.push(Call::new(".expect", vec!["\"request\"".to_owned()]));
            let receiver =
                Call::new(format!("Request::{}", route::function(&route.method)), [target]);
            lines.extend(Chain::new(receiver, calls).bound(TAB, "request"));
            lines.push("    let (status, body) = send(provider(), request).await;".to_owned());
            lines.push("    let _ = (&status, &body);".to_owned());
            lines
        }
        Act::Topic(topic) => {
            let constant = stem
                .consumers
                .iter()
                .find(|consumer| consumer.topic == *topic)
                .map_or("TOPIC", |consumer| consumer.constant.as_str());
            let provider = Call::new("provider", Vec::<Expr>::new());
            let delivery = Call::new(
                "deliver",
                [provider.into(), Expr::from(format!("topic::{constant}")), "b\"{}\"".into()],
            );
            let mut lines = Chain::new(delivery, Vec::new()).awaited().bound(TAB, "outcome");
            lines.push("    let _ = &outcome;".to_owned());
            lines
        }
        Act::Call => {
            vec!["    let provider = provider();".to_owned(), "    let _ = &provider;".to_owned()]
        }
    }
}

// The body of a handler or a consumer as scaffolded: the borrow that keeps
// the lints quiet and the marker, the one block the turn replaces.
pub fn stub(marker: &str) -> String {
    format!("    let _ = (&input, &context);\n{}", marker_line(marker))
}

// The body of a function as scaffolded: its marker alone.
pub fn marker_line(marker: &str) -> String {
    let marker = Expr::Macro(Call::new("todo!", [format!("\"{}\"", escape(marker))]));
    marker.tail(TAB).join("\n")
}

// `text` in a fence longer than any run of backticks it holds.
pub fn fenced(lang: &str, text: &str) -> String {
    let fence = Fence::around(text);
    format!("{fence}{lang}\n{}\n{fence}\n", text.trim_end_matches('\n'))
}

// Whether the method decodes its input from the body, as the SDK's `post`,
// `put`, and `patch` do.
pub fn carries_body(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH")
}

fn imports(stem: &Stem) -> Uses {
    let routed = !stem.handlers.is_empty();
    let consuming = !stem.consumers.is_empty();
    let encoding = stem.handlers.iter().any(|handler| handler.route_fn.is_some());
    let mut uses = Uses::default();
    if encoding {
        uses.add("axum", &["Json"]);
        uses.add("axum::http", &["StatusCode"]);
        uses.add("axum::response", &["IntoResponse"]);
        uses.add("axum::routing", &["MethodRouter"]);
    }
    if routed || consuming {
        let common: &[&str] =
            if consuming { &["Capabilities", "config", "topic"] } else { &["Capabilities"] };
        uses.add("common", common);
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
        uses.add("omnia_sdk::api::http", &names);
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
        uses.add("omnia_sdk::api", &api);
    }
    if encoding {
        uses.add("serde::de", &["DeserializeOwned"]);
    }
    let serializing = routed || !stem.types.is_empty();
    if serializing {
        uses.add("serde", &["Deserialize", "Serialize"]);
    } else if consuming {
        uses.add("serde", &["Deserialize"]);
    }
    uses
}

// The part's `topics(..)`: one `.route(..)` per consumer, chained over the
// router.
fn topics(stem: &Stem) -> Fn {
    let calls = stem
        .consumers
        .iter()
        .map(|consumer| {
            let topic = Call::new(
                "config::topic_for",
                ["env".to_owned(), format!("topic::{}", consumer.constant)],
            );
            let consume = Call::new("messaging::consume", [consumer.name.as_str()]);
            Call::new(".route", [topic, consume])
        })
        .collect();
    let chain = Chain::new("router", calls);
    Fn {
        vis: Vis::Crate,
        name: "topics".to_owned(),
        generics: GENERICS.to_owned(),
        params: vec!["router: messaging::Router<P>".to_owned(), "env: &str".to_owned()],
        ret: "messaging::Router<P>".to_owned(),
        body: chain.tail(TAB),
        ..Fn::default()
    }
}

// The part's `routes()`: one `.route(..)` per handler, chained over a new
// router.
fn routes(stem: &Stem) -> Fn {
    let chain = Chain::new(
        "axum::Router::new()",
        stem.handlers.iter().map(Handler::registration).collect(),
    );
    Fn {
        vis: Vis::Crate,
        name: "routes".to_owned(),
        generics: GENERICS.to_owned(),
        ret: "axum::Router<Client<P>>".to_owned(),
        body: chain.tail(TAB),
        ..Fn::default()
    }
}

// A design type's marker: the struct or enum under its signature, fenced in
// its doc.
fn design(design_type: &Type) -> Item {
    let fence = Fence::around(&design_type.signature);
    let mut docs = Doc::new([
        format!("The design type `{}`.", design_type.key),
        String::new(),
        format!("{fence}text"),
    ]);
    for line in design_type.signature.lines() {
        docs.push(line.trim_end());
    }
    docs.push(fence.to_string());
    let attrs = vec![format!("#[derive({DERIVES})]")];
    let name = design_type.name.clone();
    if design_type.is_enum {
        Item::Enum(Enum {
            docs,
            attrs,
            vis: Vis::Pub,
            name,
            ..Enum::default()
        })
    } else {
        Item::Struct(Struct {
            docs,
            attrs,
            vis: Vis::Pub,
            name,
            ..Struct::default()
        })
    }
}

// `struct Order` or `enum Status`, as the type's marker spells it.
pub fn declaration(design_type: &Type) -> String {
    let keyword = if design_type.is_enum { "enum" } else { "struct" };
    format!("{keyword} {}", design_type.name)
}

// A handler: its input over the route's parameters and its output, the
// route function where one was scaffolded, and the handler under its
// requirements, its body the marker.
fn handler(handler: &Handler, file: &mut File) {
    let route = escape(&format!("{} {}", handler.route.method, handler.route.path));
    file.push(Struct {
        docs: Doc::new([format!("The input of `{route}`.")]),
        attrs: vec!["#[derive(Debug, Deserialize)]".to_owned()],
        vis: Vis::Pub,
        name: handler.input.clone(),
        fields: handler
            .params
            .iter()
            .map(|param| {
                let wire = escape(&param.wire);
                Field {
                    docs: Doc::new([format!("The `{wire}` path parameter.")]),
                    attrs: (param.field != param.wire)
                        .then(|| format!("#[serde(rename = \"{wire}\")]"))
                        .into_iter()
                        .collect(),
                    vis: Vis::Pub,
                    name: param.field.clone(),
                    ty: "String".to_owned(),
                }
            })
            .collect(),
    });
    file.push(Struct {
        docs: Doc::new([format!("The output of `{route}`.")]),
        attrs: vec!["#[derive(Debug, Serialize)]".to_owned()],
        vis: Vis::Pub,
        name: handler.output.clone(),
        ..Struct::default()
    });
    if let Some(route_fn) = &handler.route_fn {
        let mut body = vec![
            "    handle_with(".to_owned(),
            format!("        MethodFilter::{},", handler.route.method),
            format!("        {},", handler.name),
            format!("        |raw: RawRequest<'_>| -> Result<{}, DecodeError> {{", handler.input),
            "            decode_json(raw.path_params, raw.body)".to_owned(),
            "        },".to_owned(),
        ];
        body.extend(closure(
            2 * TAB,
            &format!("|output: {}|", handler.output),
            "(StatusCode::OK, Json(output)).into_response()",
            ",",
        ));
        body.push("    )".to_owned());
        file.push(Fn {
            comments: vec![format!(
                "The `{route}` route: the handler's `Ok` answered as JSON under the status named \
                 here."
            )],
            vis: Vis::Crate,
            name: route_fn.clone(),
            generics: GENERICS.to_owned(),
            ret: "MethodRouter<Client<P>>".to_owned(),
            body,
            ..Fn::default()
        });
    }
    file.push(Fn {
        comments: stated(&handler.stated),
        vis: Vis::Crate,
        is_async: true,
        name: handler.name.clone(),
        generics: GENERICS.to_owned(),
        params: vec![format!("input: {}", handler.input), "context: Context<P>".to_owned()],
        ret: format!("omnia_sdk::Result<{}>", handler.output),
        body: lines(&stub(&handler.marker)),
        ..Fn::default()
    });
}

// A consumer: its input over the delivery, and the handler under its
// requirements, its body the marker.
fn consumer(consumer: &Consumer, file: &mut File) {
    file.push(Struct {
        docs: Doc::new([format!("A delivery of `{}`.", escape(&consumer.topic))]),
        attrs: vec!["#[derive(Debug, Deserialize)]".to_owned()],
        vis: Vis::Pub,
        name: consumer.input.clone(),
        ..Struct::default()
    });
    file.push(Fn {
        comments: stated(&consumer.stated),
        vis: Vis::Crate,
        is_async: true,
        name: consumer.name.clone(),
        generics: GENERICS.to_owned(),
        params: vec![format!("input: {}", consumer.input), "context: Context<P>".to_owned()],
        ret: "omnia_sdk::Result<()>".to_owned(),
        body: lines(&stub(&consumer.marker)),
        ..Fn::default()
    });
}

// A function under its requirement, its body the marker.
fn function(function: &Function) -> Fn {
    Fn {
        comments: stated(std::slice::from_ref(&function.stated)),
        docs: Doc::new([
            format!("The behaviour `{}` states.", function.stated.subject),
            String::new(),
            "# Errors".to_owned(),
            String::new(),
            "As the requirement states.".to_owned(),
        ]),
        vis: Vis::Pub,
        name: function.name.clone(),
        ret: "omnia_sdk::Result<()>".to_owned(),
        body: lines(&marker_line(&function.marker)),
        ..Fn::default()
    }
}

fn decode_json() -> Fn {
    Fn {
        comments: vec![
            "The JSON body with the path parameters inserted as string fields, as the".to_owned(),
            "SDK's `post` decodes an input.".to_owned(),
        ],
        name: "decode_json".to_owned(),
        generics: "<I: DeserializeOwned>".to_owned(),
        params: vec!["params: &[(String, String)]".to_owned(), "body: &[u8]".to_owned()],
        ret: "Result<I, DecodeError>".to_owned(),
        body: lines(DECODE_BODY),
        ..Fn::default()
    }
}

// `// <id> <subject>: <statement>`, one per requirement an item stands for.
fn stated(stated: &[super::Stated]) -> Vec<String> {
    stated
        .iter()
        .map(|stated| format!("{} {}: {}", stated.id, stated.subject, stated.statement))
        .collect()
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

//! Renders the files a scaffold writes: the stem's manifest and entry, the part, and its tests.

mod item;

use std::fmt::Write as _;

pub use self::item::{Call, Expr};
use self::item::{
    Chain, Const, Consts, Doc, Enum, Fence, Field, Fn, Item, Source, Struct, TAB, Uses, Variant,
    Vis, closure,
};
use super::names::{escape, flat, snake};
use super::types::{Body, DefaultFn, Member};
use super::{Act, Consumer, Function, Handler, Stated, Stem, Test, Type, trigger};
use crate::layout::{
    PROBE_DEPENDENCY, ROUTES_ARRAY_ANCHOR as ROUTES_ANCHOR, STEM_DEPENDENCIES,
    TOPICS_ARRAY_ANCHOR as TOPICS_ANCHOR,
};

const DERIVES: &str = "Debug, Clone, PartialEq, Eq, Serialize, Deserialize";
// The derives of a type a float reaches, which `Eq` cannot hold.
const DERIVES_WITHOUT_EQ: &str = "Debug, Clone, PartialEq, Serialize, Deserialize";
const GENERICS: &str = "<P: Capabilities>";

// The stem's manifest, as the layout reference spells it, its
// `[dependencies]` the layout's fixed lines and, where the slice probes, the
// probe's crate after them.
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
<dependencies>
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

pub fn manifest(stem: &Stem) -> String {
    let mut dependencies: Vec<String> =
        STEM_DEPENDENCIES.iter().map(|name| dependency_line(name)).collect();
    if stem.probing {
        dependencies.push(dependency_line(PROBE_DEPENDENCY));
    }
    MANIFEST
        .replace("<package>", &stem.package)
        .replace("<stem>", &stem.name)
        .replace("<dependencies>", &dependencies.join("\n"))
}

// The line a stem's `[dependencies]` names a crate the root manifest pins
// by.
pub fn dependency_line(name: &str) -> String {
    format!("{name}.workspace = true")
}

// The topic table's constant for `topic`, under its doc line, as the lines
// added at the end of the table.
pub fn topic_constant(constant: &str, topic: &str) -> String {
    let topic = escape(topic);
    let mut source = Source::default();
    source.push(Consts::new([Const::new(constant, "&str", format!("\"{topic}\""))
        .docs([format!("The `{topic}` topic.")])
        .vis(Vis::Pub)]));
    source.render().trim_end().to_owned()
}

// The stem's entry, as the layout reference spells it, with one part's
// lines in its lists.
pub fn entry(stem: &Stem) -> String {
    let mut source = Source::default();
    source.push(Item::ModuleDoc(Doc::new([
        format!("# {}", stem.name),
        String::new(),
        format!("The `{}` stem.", stem.name),
    ])));
    source.push(Item::Mods(vec![stem.part.clone()]));
    let mut uses = Uses::default();
    uses.add("common", &["Capabilities"]);
    uses.add("omnia_sdk::api", &["Client", "messaging"]);
    source.push(uses);
    let routes = (!stem.handlers.is_empty()).then(|| format!("{}::routes(),", stem.part));
    source.push(
        Fn::new("router")
            .docs(["The stem's routes, merged onto the root router."])
            .vis(Vis::Pub)
            .generics(GENERICS)
            .returns("axum::Router<Client<P>>")
            .body(fold("common::routers([", ROUTES_ANCHOR, routes)),
    );
    let topics = (!stem.consumers.is_empty()).then(|| format!("{}::topics,", stem.part));
    source.push(
        Fn::new("topics")
            .docs(["The stem's topics, registered on the root messaging router under `env`."])
            .attr("#[must_use]")
            .vis(Vis::Pub)
            .generics(GENERICS)
            .params(["router: messaging::Router<P>", "env: &str"])
            .returns("messaging::Router<P>")
            .body(fold("common::topics(router, env).parts([", TOPICS_ANCHOR, topics)),
    );
    source.render()
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
    let mut source = Source::default();
    source.push(Item::ModuleDoc(Doc::new(stem.doc.iter().cloned())));
    let uses = imports(stem);
    if !uses.is_empty() {
        source.push(uses);
    }
    if !stem.handlers.is_empty() {
        let paths = stem.paths.iter().map(|(constant, path)| {
            Const::new(constant, "&str", format!("\"{}\"", escape(path))).vis(Vis::Crate)
        });
        source.push(Consts::new(paths).comment("The paths the part serves."));
        source.push(routes(stem));
    }
    if !stem.consumers.is_empty() {
        source.push(topics(stem));
    }
    for design_type in &stem.types {
        design(design_type, &mut source);
    }
    for handler in &stem.handlers {
        self::handler(handler, &mut source);
    }
    for consumer in &stem.consumers {
        self::consumer(consumer, &mut source);
    }
    for function in &stem.functions {
        source.push(self::function(function));
    }
    if stem.handlers.iter().any(|handler| handler.route_fn.is_some()) {
        source.push(decode_json());
    }
    source.render()
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

    let mut source = Source::default();
    source.push(Item::ModuleDoc(Doc::new([format!(
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
        uses.add("serde_json", &["Value"]);
        uses.add("tower", &["ServiceExt as _"]);
    }
    source.push(uses);

    source.push(provider());
    if sending {
        source.push(send(&ident));
    }
    if delivering {
        source.push(deliver(&ident));
    }
    for test in &stem.scenarios {
        source.push(
            scenario(stem, test).comments(steps(test)).attr("#[tokio::test]").attr(ignored(test)),
        );
    }
    source.render()
}

// The doubles every scenario starts from.
fn provider() -> Fn {
    Fn::new("provider")
        .comments([
            "The doubles every scenario starts from: set a setting or a response on them in the \
             arrangement.",
        ])
        .returns("Provider")
        .body([
            "    Provider::default().config(MapConfig::default().with([(config::ENV, \"test\")]))",
        ])
}

// One request through the router of the stem `ident` names, its body read
// as JSON so a test asserts fields and never bytes.
fn send(ident: &str) -> Fn {
    Fn::new("send")
        .comments([
            "One request through the stem's router, answered as its status and its body as JSON:",
            "`Value::Null` for an empty body, the text as a `Value::String` where it is not JSON.",
        ])
        .asynchronous()
        .params(["provider: Provider", "request: Request<Body>"])
        .returns("(StatusCode, Value)")
        .body([
            format!(
                "    let router = {ident}::router().with_state(Client::new(\"test\", provider));"
            ),
            "    let response = router.oneshot(request).await.expect(\"response\");".to_owned(),
            "    let status = response.status();".to_owned(),
            "    let body = to_bytes(response.into_body(), usize::MAX).await.expect(\"body\");"
                .to_owned(),
            "    let body = if body.is_empty() {".to_owned(),
            "        Value::Null".to_owned(),
            "    } else {".to_owned(),
            "        serde_json::from_slice(&body)".to_owned(),
            "            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()))"
                .to_owned(),
            "    };".to_owned(),
            "    (status, body)".to_owned(),
        ])
}

// One delivery through the topics of the stem `ident` names.
fn deliver(ident: &str) -> Fn {
    Fn::new("deliver")
        .comments(["One delivery through the stem's topics, under the test environment."])
        .asynchronous()
        .params(["provider: Provider", "topic: &str", "payload: &[u8]"])
        .returns("Result<(), DeliveryError>")
        .body([
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
        ])
}

// A scaffolded test from its `#[ignore]` line to its closing brace: the one
// block the turn replaces, so the brief quotes it as the `find`.
pub fn test_block(stem: &Stem, test: &Test) -> String {
    let mut source = Source::default();
    source.push(scenario(stem, test).attr(ignored(test)));
    source.render().trim_end().to_owned()
}

// A scenario's test as the turn finds it: its act laid from the typed route
// or topic its requirement carries, its marker last.
fn scenario(stem: &Stem, test: &Test) -> Fn {
    let mut body = act(stem, test);
    body.push(marker_line(&test.marker));
    Fn::new(&test.name).asynchronous().body(body)
}

fn ignored(test: &Test) -> String {
    format!("#[ignore = \"{}\"]", escape(&test.ignore))
}

// The scenario's steps, quoted above its test.
fn steps(test: &Test) -> Vec<String> {
    let mut lines = vec![format!("{} {}: {}", test.id, test.subject, flat(&test.scenario.name))];
    lines.extend(test.scenario.given.iter().map(|given| format!("  Given {}", flat(given))));
    lines.push(format!("  When {}", flat(&test.scenario.when)));
    lines.push(format!("  Then {}", flat(&test.scenario.then)));
    lines.extend(test.scenario.and.iter().map(|and| format!("  And {}", flat(and))));
    lines
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
                calls.push(Call::new(".header", ["CONTENT_TYPE", "\"application/json\""]));
                calls.push(Call::new(".body", ["Body::from(\"{}\")"]));
            } else {
                calls.push(Call::new(".body", ["Body::empty()"]));
            }
            calls.push(Call::new(".expect", ["\"request\""]));
            let receiver =
                Call::new(format!("Request::{}", trigger::method_fn(&route.method)), [target]);
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
    if stem.types.iter().any(|design_type| design_type.body.uses_map()) {
        uses.add("std::collections", &["BTreeMap"]);
    }
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
            .map(|handler| trigger::method_fn(&handler.route.method))
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
    Fn::new("topics")
        .vis(Vis::Crate)
        .generics(GENERICS)
        .params(["router: messaging::Router<P>", "env: &str"])
        .returns("messaging::Router<P>")
        .body(Chain::new("router", calls).tail(TAB))
}

// The part's `routes()`: one `.route(..)` per handler, chained over a new
// router.
fn routes(stem: &Stem) -> Fn {
    let chain = Chain::new(
        "axum::Router::new()",
        stem.handlers.iter().map(Handler::registration).collect(),
    );
    Fn::new("routes")
        .vis(Vis::Crate)
        .generics(GENERICS)
        .returns("axum::Router<Client<P>>")
        .body(chain.tail(TAB))
}

// A design type as the scaffold lays it: the struct or enum under its
// signature, fenced in its doc, with the fields or variants the signature
// was read into and the fn each `serde(default)` names after it, or the
// empty marker where the signature was not read.
fn design(design_type: &Type, source: &mut Source) {
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
    let name = design_type.name.as_str();
    match &design_type.body {
        Body::Struct {
            generics,
            attrs,
            fields,
            defaults,
            eq,
            ..
        } => {
            let derives = if *eq { DERIVES } else { DERIVES_WITHOUT_EQ };
            source.push(
                Struct::new(name)
                    .docs(docs)
                    .attr(format!("#[derive({derives})]"))
                    .attrs(attrs.iter().cloned())
                    .vis(Vis::Pub)
                    .generics(generics)
                    .fields(fields.iter().map(laid_field)),
            );
            for default in defaults {
                source.push(default_fn(default));
            }
        }
        Body::Enum { variants, numeric } => {
            if *numeric {
                docs.push("");
                docs.push(
                    "The source enumerates by number; the wire carries each variant's name here.",
                );
            }
            source.push(
                Enum::new(name)
                    .docs(docs)
                    .attr(format!("#[derive({DERIVES})]"))
                    .vis(Vis::Pub)
                    .variants(variants.iter().map(laid_variant)),
            );
        }
        Body::Marker { is_enum: true } => {
            source.push(
                Enum::new(name).docs(docs).attr(format!("#[derive({DERIVES})]")).vis(Vis::Pub),
            );
        }
        Body::Marker { is_enum: false } => {
            source.push(
                Struct::new(name).docs(docs).attr(format!("#[derive({DERIVES})]")).vis(Vis::Pub),
            );
        }
    }
}

fn laid_field(member: &Member) -> Field {
    Field::new(&member.name, &member.ty)
        .docs(member.docs.clone())
        .attrs(member.attrs.iter().cloned())
        .vis(Vis::Pub)
}

fn laid_variant(member: &Member) -> Variant {
    Variant::new(&member.name).docs(member.docs.clone()).attrs(member.attrs.iter().cloned())
}

// The fn a field's `serde(default)` names, `const` where its value allows.
fn default_fn(default: &DefaultFn) -> Fn {
    let item = Fn::new(&default.name).returns(&default.ret).body([format!(
        "{}{}",
        " ".repeat(TAB),
        default.value
    )]);
    if default.constant { item.constant() } else { item }
}

// A handler: its input over the route's parameters and its output, the
// route function where one was scaffolded, and the handler under its
// requirements, its body the marker.
fn handler(handler: &Handler, source: &mut Source) {
    let route = escape(&format!("{} {}", handler.route.method, handler.route.path));
    let params = handler.params.iter().map(|param| {
        let wire = escape(&param.wire);
        Field::new(&param.field, "String")
            .docs([format!("The `{wire}` path parameter.")])
            .attrs((param.field != param.wire).then(|| format!("#[serde(rename = \"{wire}\")]")))
            .vis(Vis::Pub)
    });
    source.push(
        Struct::new(&handler.input)
            .docs([format!("The input of `{route}`.")])
            .attr("#[derive(Debug, Deserialize)]")
            .vis(Vis::Pub)
            .fields(params),
    );
    source.push(
        Struct::new(&handler.output)
            .docs([format!("The output of `{route}`.")])
            .attr("#[derive(Debug, Serialize)]")
            .vis(Vis::Pub),
    );
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
        source.push(
            Fn::new(route_fn)
                .comments([format!(
                    "The `{route}` route: the handler's `Ok` answered as JSON under the status \
                     named here."
                )])
                .vis(Vis::Crate)
                .generics(GENERICS)
                .returns("MethodRouter<Client<P>>")
                .body(body),
        );
    }
    source.push(
        Fn::new(&handler.name)
            .comments(stated(&handler.stated))
            .vis(Vis::Crate)
            .asynchronous()
            .generics(GENERICS)
            .params([format!("input: {}", handler.input), "context: Context<P>".to_owned()])
            .returns(format!("omnia_sdk::Result<{}>", handler.output))
            .body(stub(&handler.marker).lines()),
    );
}

// A consumer: its input over the delivery, and the handler under its
// requirements, its body the marker.
fn consumer(consumer: &Consumer, source: &mut Source) {
    source.push(
        Struct::new(&consumer.input)
            .docs([format!("A delivery of `{}`.", escape(&consumer.topic))])
            .attr("#[derive(Debug, Deserialize)]")
            .vis(Vis::Pub),
    );
    source.push(
        Fn::new(&consumer.name)
            .comments(stated(&consumer.stated))
            .vis(Vis::Crate)
            .asynchronous()
            .generics(GENERICS)
            .params([format!("input: {}", consumer.input), "context: Context<P>".to_owned()])
            .returns("omnia_sdk::Result<()>")
            .body(stub(&consumer.marker).lines()),
    );
}

// A function under its requirement, its body the marker.
fn function(function: &Function) -> Fn {
    Fn::new(&function.name)
        .comments(stated(std::slice::from_ref(&function.stated)))
        .docs([
            format!("The behaviour `{}` states.", function.stated.subject),
            String::new(),
            "# Errors".to_owned(),
            String::new(),
            "As the requirement states.".to_owned(),
        ])
        .vis(Vis::Pub)
        .returns("omnia_sdk::Result<()>")
        .body(marker_line(&function.marker).lines())
}

fn decode_json() -> Fn {
    Fn::new("decode_json")
        .comments([
            "The JSON body with the path parameters inserted as string fields, as the",
            "SDK's `post` decodes an input.",
        ])
        .generics("<I: DeserializeOwned>")
        .params(["params: &[(String, String)]", "body: &[u8]"])
        .returns("Result<I, DecodeError>")
        .body(DECODE_BODY.lines())
}

// `// <id> <subject>: <statement>`, one per requirement an item stands for.
fn stated(stated: &[Stated]) -> Vec<String> {
    stated
        .iter()
        .map(|stated| format!("{} {}: {}", stated.id, stated.subject, stated.statement))
        .collect()
}

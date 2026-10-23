//! Renders the files a scaffold writes: the stem's manifest and entry, the part, and its tests.

mod item;

use std::fmt::Write as _;

use self::item::{
    Call, Chain, Const, Consts, Doc, Enum, Expr, Fence, Field, Fn, Item, Source, Struct, TAB, Uses,
    Variant, Vis, block, closure, pad,
};
use super::names::{escape, flat, snake};
use super::stem::{Act, Consumer, Function, Handler, Kind, Stated, Stem, Test, Type};
use super::trigger;
use super::types::{Body, DefaultFn, LaidField, LaidVariant};
use crate::layout::{
    PROBE_DEPENDENCY, ROUTES_ARRAY_ANCHOR as ROUTES_ANCHOR, STEM_DEPENDENCIES,
    TOPICS_ARRAY_ANCHOR as TOPICS_ANCHOR,
};

const DERIVES: &str = "Debug, Clone, PartialEq, Eq, Serialize, Deserialize";
// The derives of a type a float reaches, which `Eq` cannot hold.
const DERIVES_WITHOUT_EQ: &str = "Debug, Clone, PartialEq, Serialize, Deserialize";
const GENERICS: &str = "<P: Capabilities>";

// The stem's manifest, as the layout reference spells it, its
// `[dependencies]` the layout's fixed lines and, where the slice probes or
// reaches a SQL store, the SQL crate after them.
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

// The bodies of the decoders a pre-rendered route function reads the input
// with, as lines relative to the block: what the SDK's `post` and `get` do,
// spelled in the part so the encoder beside them is the one place a status
// changes. The SDK's form decoder is private, so a query is read as string
// fields over `serde_json`, which coerces nothing, and a query field is a
// `String`.
const DECODE_JSON_BODY: &str = r#"let mut value = if body.is_empty() {
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
const DECODE_QUERY_BODY: &str = r#"let mut object = serde_json::Map::new();
for (key, param) in params {
    object.insert(key.clone(), serde_json::Value::String(param.clone()));
}
for pair in query.unwrap_or_default().split('&').filter(|pair| !pair.is_empty()) {
    let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
    object.insert(unescape(key)?, serde_json::Value::String(unescape(value)?));
}
serde_json::from_value(serde_json::Value::Object(object))
    .map_err(|error| DecodeError::new(error.to_string()))"#;
const UNESCAPE_BODY: &str = r#"let malformed = || DecodeError::new(format!("malformed query string: `{token}`"));
let hex = |c: Option<char>| -> Option<u8> { u8::try_from(c?.to_digit(16)?).ok() };
let mut bytes = Vec::with_capacity(token.len());
let mut chars = token.chars();
while let Some(c) = chars.next() {
    match c {
        '+' => bytes.push(b' '),
        '%' => {
            let (Some(high), Some(low)) = (hex(chars.next()), hex(chars.next())) else {
                return Err(malformed());
            };
            bytes.push(high * 16 + low);
        }
        other => bytes.extend_from_slice(other.encode_utf8(&mut [0; 4]).as_bytes()),
    }
}
String::from_utf8(bytes).map_err(|error| DecodeError::new(error.to_string()))"#;

impl Stem {
    pub(super) fn render(&self, kind: Kind) -> String {
        match kind {
            Kind::Manifest => self.render_manifest(),
            Kind::Entry => self.render_entry(),
            Kind::Module => self.render_module(),
            Kind::Tests => self.render_tests(),
        }
    }

    fn render_manifest(&self) -> String {
        let mut dependencies: Vec<String> =
            STEM_DEPENDENCIES.iter().map(|name| dependency_line(name)).collect();
        if self.sql {
            dependencies.push(dependency_line(PROBE_DEPENDENCY));
        }
        MANIFEST
            .replace("<package>", &self.package)
            .replace("<stem>", &self.name)
            .replace("<dependencies>", &dependencies.join("\n"))
    }

    // The stem's entry, as the layout reference spells it, with one part's
    // lines in its lists.
    fn render_entry(&self) -> String {
        let mut source = Source::default();
        source.push(Doc::new([
            format!("# {}", self.name),
            String::new(),
            format!("The `{}` stem.", self.name),
        ]));
        source.push(Item::Mods(vec![self.part.clone()]));
        let mut uses = Uses::default();
        uses.add("common", &["Capabilities"]);
        uses.add("omnia_sdk::api", &["Client", "messaging"]);
        source.push(uses);
        let routes = (!self.handlers.is_empty()).then(|| format!("{}::routes(),", self.part));
        source.push(
            Fn::new("router")
                .docs(["The stem's routes, merged onto the root router."])
                .vis(Vis::Pub)
                .generics(GENERICS)
                .returns("axum::Router<Client<P>>")
                .body(fold("common::routers([", ROUTES_ANCHOR, routes)),
        );
        let topics = (!self.consumers.is_empty()).then(|| format!("{}::topics,", self.part));
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

    fn render_module(&self) -> String {
        let mut source = Source::default();
        source.push(Doc::new(self.doc.iter().cloned()));
        let uses = self.imports();
        if !uses.is_empty() {
            source.push(uses);
        }
        if !self.handlers.is_empty() {
            let paths = self.paths.iter().map(|(constant, path)| {
                Const::new(constant, "&str", format!("\"{}\"", escape(path))).vis(Vis::Crate)
            });
            source.push(Consts::new(paths).comment("The paths the part serves."));
            source.push(self.routes_fn());
        }
        if !self.consumers.is_empty() {
            source.push(self.topics_fn());
        }
        for design_type in &self.types {
            design_type.render(&mut source);
        }
        for handler in &self.handlers {
            handler.render(&mut source);
        }
        for consumer in &self.consumers {
            consumer.render(&mut source);
        }
        for function in &self.functions {
            source.push(function.render());
        }
        let encoded = self.handlers.iter().filter(|handler| handler.route_fn.is_some());
        let (json, query): (Vec<_>, Vec<_>) =
            encoded.partition(|handler| carries_body(&handler.route.method));
        if !json.is_empty() {
            source.push(decode_json());
        }
        if !query.is_empty() {
            source.push(decode_query());
            source.push(unescape());
        }
        source.render()
    }

    // The test file: the prelude the scenarios act through, each helper
    // only where a test calls it, then one ignored test per scenario.
    fn render_tests(&self) -> String {
        let sending = self.scenarios.iter().any(|test| matches!(test.act, Act::Route(_)));
        let delivering = self.scenarios.iter().any(|test| matches!(test.act, Act::Topic(_)));
        let with_body = self
            .scenarios
            .iter()
            .any(|test| matches!(&test.act, Act::Route(route) if carries_body(&route.method)));
        let ident = self.package.replace('-', "_");

        let mut source = Source::default();
        source.push(Doc::new([format!(
            "The acceptance scenarios of the `{}` part of the `{}` stem, over the guest doubles.",
            self.part, self.name
        )]));
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
        for test in &self.scenarios {
            source.push(
                test.render(self)
                    .comments(test.steps())
                    .attr("#[tokio::test]")
                    .attr(test.ignored()),
            );
        }
        source.render()
    }

    fn imports(&self) -> Uses {
        let routed = !self.handlers.is_empty();
        let consuming = !self.consumers.is_empty();
        let encoding = self.handlers.iter().any(|handler| handler.route_fn.is_some());
        let mut uses = Uses::default();
        if self.types.iter().any(|design_type| design_type.body.uses_map()) {
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
            let mut names: Vec<&str> = self
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
        let serializing = routed || !self.types.is_empty();
        if serializing {
            uses.add("serde", &["Deserialize", "Serialize"]);
        } else if consuming {
            uses.add("serde", &["Deserialize"]);
        }
        uses
    }

    // The part's `topics(..)`: one `.route(..)` per consumer, chained over
    // the router.
    fn topics_fn(&self) -> Fn {
        let calls = self
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
            .body(Chain::new("router", calls).tail(0))
    }

    // The part's `routes()`: one `.route(..)` per handler, chained over a
    // new router.
    fn routes_fn(&self) -> Fn {
        let chain = Chain::new(
            "axum::Router::new()",
            self.handlers.iter().map(Handler::registration).collect(),
        );
        Fn::new("routes")
            .vis(Vis::Crate)
            .generics(GENERICS)
            .returns("axum::Router<Client<P>>")
            .body(chain.tail(0))
    }
}

impl Handler {
    // The `.route(..)` call that registers the handler: its route function
    // where one was scaffolded, else the SDK's method fn over it.
    fn registration(&self) -> Call {
        let service = self.route_fn.as_ref().map_or_else(
            || Call::new(trigger::method_fn(&self.route.method), [self.name.as_str()]),
            |route_fn| Call::new(route_fn.as_str(), Vec::<Expr>::new()),
        );
        Call::new(".route", [Expr::from(self.constant.as_str()), service.into()])
    }

    // Its input over the route's parameters and its output, the route
    // function where one was scaffolded, and the handler under its
    // requirements, its body the marker.
    fn render(&self, source: &mut Source) {
        let route = escape(&format!("{} {}", self.route.method, self.route.path));
        let params = self.params.iter().map(|param| {
            let wire = escape(&param.wire);
            Field::new(&param.field, "String")
                .docs([format!("The `{wire}` path parameter.")])
                .attrs(
                    (param.field != param.wire).then(|| format!("#[serde(rename = \"{wire}\")]")),
                )
                .vis(Vis::Pub)
        });
        source.push(
            Struct::new(&self.input)
                .docs([format!("The input of `{route}`.")])
                .attr("#[derive(Debug, Deserialize)]")
                .vis(Vis::Pub)
                .fields(params),
        );
        source.push(
            Struct::new(&self.output)
                .docs([format!("The output of `{route}`.")])
                .attr("#[derive(Debug, Serialize)]")
                .vis(Vis::Pub),
        );
        if let Some(route_fn) = &self.route_fn {
            let decode = if carries_body(&self.route.method) {
                "decode_json(raw.path_params, raw.body)"
            } else {
                "decode_query(raw.path_params, raw.query)"
            };
            let arg = pad(TAB);
            let mut body = vec![
                "handle_with(".to_owned(),
                format!("{arg}MethodFilter::{},", self.route.method),
                format!("{arg}{},", self.name),
                format!("{arg}|raw: RawRequest<'_>| -> Result<{}, DecodeError> {{", self.input),
                format!("{}{decode}", pad(2 * TAB)),
                format!("{arg}}},"),
            ];
            body.extend(closure(
                TAB,
                &format!("|output: {}|", self.output),
                "(StatusCode::OK, Json(output)).into_response()",
                ",",
            ));
            body.push(")".to_owned());
            source.push(
                Fn::new(route_fn)
                    .comments([format!(
                        "The `{route}` route: the handler's `Ok` answered as JSON under the \
                         status named here."
                    )])
                    .vis(Vis::Crate)
                    .generics(GENERICS)
                    .returns("MethodRouter<Client<P>>")
                    .body(body),
            );
        }
        source.push(
            Fn::new(&self.name)
                .comments(stated(&self.stated))
                .vis(Vis::Crate)
                .asynchronous()
                .generics(GENERICS)
                .params([format!("input: {}", self.input), "context: Context<P>".to_owned()])
                .returns(format!("omnia_sdk::Result<{}>", self.output))
                .body(stub_lines(&self.marker)),
        );
    }
}

impl Consumer {
    // Its input over the delivery, and the handler under its requirements,
    // its body the marker.
    fn render(&self, source: &mut Source) {
        source.push(
            Struct::new(&self.input)
                .docs([format!("A delivery of `{}`.", escape(&self.topic))])
                .attr("#[derive(Debug, Deserialize)]")
                .vis(Vis::Pub),
        );
        source.push(
            Fn::new(&self.name)
                .comments(stated(&self.stated))
                .vis(Vis::Crate)
                .asynchronous()
                .generics(GENERICS)
                .params([format!("input: {}", self.input), "context: Context<P>".to_owned()])
                .returns("omnia_sdk::Result<()>")
                .body(stub_lines(&self.marker)),
        );
    }
}

impl Function {
    // The function from its signature to its closing brace, the comment and
    // doc lines above it standing: the one block the turn replaces, since
    // the behaviour decides the signature too.
    pub(super) fn block(&self) -> String {
        let mut source = Source::default();
        source.push(self.signature());
        source.render().trim_end().to_owned()
    }

    // The function under its requirement, its body the marker.
    fn render(&self) -> Fn {
        self.signature().comments(stated(std::slice::from_ref(&self.stated))).docs([
            format!("The behaviour `{}` states.", self.stated.subject),
            String::new(),
            "# Errors".to_owned(),
            String::new(),
            "As the requirement states.".to_owned(),
        ])
    }

    fn signature(&self) -> Fn {
        Fn::new(&self.name)
            .vis(Vis::Pub)
            .returns("omnia_sdk::Result<()>")
            .body(marker_lines(&self.marker))
    }
}

impl Type {
    // The struct or enum under its signature, fenced in its doc, with the
    // fields or variants the signature was read into and the fn each
    // `serde(default)` names after it, or the empty marker where the
    // signature was not read.
    fn render(&self, source: &mut Source) {
        let fence = Fence::around(&self.signature);
        let mut docs = Doc::new([
            format!("The design type `{}`.", self.key),
            String::new(),
            format!("{fence}text"),
        ]);
        for line in self.signature.lines() {
            docs.push(line.trim_end());
        }
        docs.push(fence.to_string());
        let name = self.name.as_str();
        match &self.body {
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
                        "The source enumerates by number; the wire carries each variant's name \
                         here.",
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
                    Struct::new(name)
                        .docs(docs)
                        .attr(format!("#[derive({DERIVES})]"))
                        .vis(Vis::Pub),
                );
            }
        }
    }
}

impl Test {
    // The test from its `#[ignore]` line to its closing brace: the one
    // block the turn replaces, so the brief quotes it under the marker's
    // key.
    pub(super) fn block(&self, stem: &Stem) -> String {
        let mut source = Source::default();
        source.push(self.render(stem).attr(self.ignored()));
        source.render().trim_end().to_owned()
    }

    // The test as the turn finds it: its act laid from the typed route or
    // topic its requirement carries, its marker last.
    fn render(&self, stem: &Stem) -> Fn {
        let mut body = self.act_lines(stem);
        body.extend(marker_lines(&self.marker));
        Fn::new(&self.name).asynchronous().body(body)
    }

    fn ignored(&self) -> String {
        format!("#[ignore = \"{}\"]", escape(&self.ignore))
    }

    // The scenario's steps, quoted above its test.
    fn steps(&self) -> Vec<String> {
        let mut lines =
            vec![format!("{} {}: {}", self.id, self.subject, flat(&self.scenario.name))];
        lines.extend(self.scenario.given.iter().map(|given| format!("  Given {}", flat(given))));
        lines.push(format!("  When {}", flat(&self.scenario.when)));
        lines.push(format!("  Then {}", flat(&self.scenario.then)));
        lines.extend(self.scenario.and.iter().map(|and| format!("  And {}", flat(and))));
        lines
    }

    // The act of the scenario, laid from the typed route or topic its
    // requirement carries: the request sent, the delivery made, or the
    // provider alone.
    fn act_lines(&self, stem: &Stem) -> Vec<String> {
        match &self.act {
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
                    lines.push(format!("let {field} = \"<{}>\";", escape(wire)));
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
                lines.extend(Chain::new(receiver, calls).bound(0, "request"));
                lines.push("let (status, body) = send(provider(), request).await;".to_owned());
                lines.push("let _ = (&status, &body);".to_owned());
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
                let mut lines = Chain::new(delivery, Vec::new()).awaited().bound(0, "outcome");
                lines.push("let _ = &outcome;".to_owned());
                lines
            }
            Act::Call => {
                vec!["let provider = provider();".to_owned(), "let _ = &provider;".to_owned()]
            }
        }
    }
}

// The line a stem's `[dependencies]` names a crate the root manifest pins
// by.
pub(super) fn dependency_line(name: &str) -> String {
    format!("{name}.workspace = true")
}

// The topic table's constant for `topic`, under its doc line, as the lines
// added at the end of the table.
pub(super) fn topic_constant(constant: &str, topic: &str) -> String {
    let topic = escape(topic);
    let mut source = Source::default();
    source.push(Consts::new([Const::new(constant, "&str", format!("\"{topic}\""))
        .docs([format!("The `{topic}` topic.")])
        .vis(Vis::Pub)]));
    source.render().trim_end().to_owned()
}

// The body of an entry's fold: its array opened under the anchor the
// scaffold's lines and the engine's union merge read, the part's entry
// beneath where the part has one.
fn fold(opener: &str, anchor: &str, entry: Option<String>) -> Vec<String> {
    let mut lines = vec![opener.to_owned(), format!("{}{anchor}", pad(TAB))];
    lines.extend(entry.map(|entry| format!("{}{entry}", pad(TAB))));
    lines.push("])".to_owned());
    lines
}

// The doubles every scenario starts from.
fn provider() -> Fn {
    Fn::new("provider")
        .comments([
            "The doubles every scenario starts from: set a setting or a response on them in the \
             arrangement.",
        ])
        .returns("Provider")
        .body(["Provider::default().config(MapConfig::default().with([(config::ENV, \"test\")]))"])
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
            format!("let router = {ident}::router().with_state(Client::new(\"test\", provider));"),
            "let response = router.oneshot(request).await.expect(\"response\");".to_owned(),
            "let status = response.status();".to_owned(),
            "let body = to_bytes(response.into_body(), usize::MAX).await.expect(\"body\");"
                .to_owned(),
            "let body = if body.is_empty() {".to_owned(),
            "    Value::Null".to_owned(),
            "} else {".to_owned(),
            "    serde_json::from_slice(&body)".to_owned(),
            "        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()))"
                .to_owned(),
            "};".to_owned(),
            "(status, body)".to_owned(),
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
                "let router = {ident}::topics(Router::new(Client::new(\"test\", provider)), \
                 \"test\");"
            ),
            "let delivery = Delivery {".to_owned(),
            "    topic: Some(config::topic_for(\"test\", topic)),".to_owned(),
            "    payload: payload.to_vec(),".to_owned(),
            "    ..Delivery::default()".to_owned(),
            "};".to_owned(),
            "router.handle(delivery).await".to_owned(),
        ])
}

// The body of a handler or a consumer as the file holds it: the borrow
// that keeps the lints quiet and the marker, the one block the turn
// replaces.
pub(super) fn stub(marker: &str) -> String {
    block(stub_lines(marker)).collect::<Vec<_>>().join("\n")
}

fn stub_lines(marker: &str) -> Vec<String> {
    let mut lines = vec!["let _ = (&input, &context);".to_owned()];
    lines.extend(marker_lines(marker));
    lines
}

// The body of a function as scaffolded: its marker alone.
fn marker_lines(marker: &str) -> Vec<String> {
    Expr::Macro(Call::new("todo!", [format!("\"{}\"", escape(marker))])).tail(0)
}

// `text` in a fence longer than any run of backticks it holds.
pub(super) fn fenced(lang: &str, text: &str) -> String {
    let fence = Fence::around(text);
    format!("{fence}{lang}\n{}\n{fence}\n", text.trim_end_matches('\n'))
}

// Whether the method decodes its input from the body, as the SDK's `post`,
// `put`, and `patch` do.
fn carries_body(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH")
}

fn laid_field(field: &LaidField) -> Field {
    Field::new(&field.name, &field.ty)
        .docs(field.docs.clone())
        .attrs(field.attrs.iter().cloned())
        .vis(Vis::Pub)
}

fn laid_variant(variant: &LaidVariant) -> Variant {
    Variant::new(&variant.name).docs(variant.docs.clone()).attrs(variant.attrs.iter().cloned())
}

// The fn a field's `serde(default)` names, `const` where its value allows.
fn default_fn(default: &DefaultFn) -> Fn {
    let item = Fn::new(&default.name).returns(&default.ret).body([default.value.as_str()]);
    if default.constant { item.constant() } else { item }
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
        .body(DECODE_JSON_BODY.lines())
}

fn decode_query() -> Fn {
    Fn::new("decode_query")
        .comments([
            "The path parameters and the query string together as string fields, as the",
            "SDK's `get` decodes an input: a field the query fills is a `String`, or",
            "deserialises from one.",
        ])
        .generics("<I: DeserializeOwned>")
        .params(["params: &[(String, String)]", "query: Option<&str>"])
        .returns("Result<I, DecodeError>")
        .body(DECODE_QUERY_BODY.lines())
}

fn unescape() -> Fn {
    Fn::new("unescape")
        .comments(["One form-encoded token: `+` a space, `%XX` the byte it spells."])
        .params(["token: &str"])
        .returns("Result<String, DecodeError>")
        .body(UNESCAPE_BODY.lines())
}

// `// <id> <subject>: <statement>`, one per requirement an item stands for.
fn stated(stated: &[Stated]) -> Vec<String> {
    stated
        .iter()
        .map(|stated| format!("{} {}: {}", stated.id, stated.subject, stated.statement))
        .collect()
}

//! Renders the files a scaffold writes: the stem's manifest and entry, the part, and its tests.

mod item;

use std::fmt::Write as _;

use self::item::{
    BODY_WIDTH, Call, Chain, Const, Consts, Doc, Enum, Expr, Fence, Field, Fn, Item, Source,
    Struct, Uses, Variant, Vis, block,
};
pub use self::item::{MAX_WIDTH, TAB, pad};
use super::lists::{Place, insert};
use super::names::{escape, flat, snake};
use super::settings::Settings;
use super::stem::{Act, Consumer, Function, Handler, Kind, Stated, Stem, Test, Type};
use super::trigger;
use super::types::{Body, DefaultFn, LaidField, LaidVariant};
use crate::layout::{
    HANDLER_LIST_ANCHOR, STEM_DEPENDENCIES, STEM_MANIFEST_HEADER, TOPIC_DEPENDENCY,
};
use crate::syntax;

const DERIVES: &str = "Debug, Clone, PartialEq, Eq, Serialize, Deserialize";
// The derives of a type a float reaches, which `Eq` cannot hold.
const DERIVES_WITHOUT_EQ: &str = "Debug, Clone, PartialEq, Serialize, Deserialize";
// The bound every scaffolded handler and consumer opens with: the turn
// narrows it to the capabilities the body reaches.
const BOUND: &str = "P: Capabilities";
// The value a seeded setting holds until the turn fills it.
const PLACEHOLDER: &str = "<value>";

impl Stem {
    pub fn render(&self, kind: Kind) -> String {
        match kind {
            Kind::Manifest => self.render_manifest(),
            Kind::Entry => self.render_entry(),
            Kind::Module => self.render_module(),
            Kind::Tests => self.render_tests(),
        }
    }

    // The manifest: the header, then one line per crate the stem's files
    // use, the topic table's crate among them where a part names a topic.
    fn render_manifest(&self) -> String {
        let mut names: Vec<&str> = STEM_DEPENDENCIES.to_vec();
        if !self.consumers.is_empty() {
            names.push(TOPIC_DEPENDENCY);
        }
        names.sort_unstable();
        let dependencies: Vec<String> = names.iter().map(|name| dependency_line(name)).collect();
        format!(
            "{}\n{}\n",
            STEM_MANIFEST_HEADER.replace("<package>", &self.package).replace("<stem>", &self.name),
            dependencies.join("\n")
        )
    }

    // The stem's entry, as the layout reference spells it, with the part in
    // its list where the part registers anything.
    fn render_entry(&self) -> String {
        let mut source = Source::default();
        source.push(Doc::new([
            format!("# {}", self.name),
            String::new(),
            format!("The `{}` stem.", self.name),
        ]));
        source.push(Item::Mods(vec![self.part.clone()]));
        let mut uses = Uses::default();
        uses.add("omnia_sdk", &["Capabilities"]);
        uses.add("omnia_sdk::api", &["Handlers"]);
        source.push(uses);
        source.push(
            Fn::new("handlers")
                .docs(["The stem's handlers, merged onto the root routers."])
                .vis(Vis::Pub)
                .generics("<P: Capabilities>")
                .returns("Handlers<P>")
                .body(["Handlers::from([])"]),
        );
        let text = source.render();
        if !self.registers() {
            return text;
        }
        let line = format!("{}::handlers()", self.part);
        insert(&text, &line, &Place::After(HANDLER_LIST_ANCHOR)).unwrap_or(text)
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
        }
        if self.registers() {
            source.push(self.handlers_fn());
        }
        if let Some(settings) = &self.settings {
            source.push(settings.render());
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
        source.render()
    }

    // The test file: the prelude the scenarios act through, each helper
    // only where a test calls it, then one ignored test per scenario.
    fn render_tests(&self) -> String {
        let sending = self.sending();
        let delivering = self.delivering();
        let ident = self.package.replace('-', "_");

        let mut source = Source::default();
        source.push(Doc::new([format!(
            "The acceptance scenarios of the `{}` part of the `{}` stem, over the guest doubles.",
            self.part, self.name
        )]));
        source.push(self.test_imports());

        source.push(provider(self.settings.as_ref()));
        if sending || delivering {
            source.push(router(&ident));
        }
        if sending {
            source.push(call());
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
        let mut uses = Uses::default();
        if self.types.iter().any(|design_type| design_type.body.uses_map()) {
            uses.add("std::collections", &["BTreeMap"]);
        }
        if routed {
            uses.add("axum", &["Json"]);
            if self.handlers.iter().any(|handler| !handler.params.is_empty()) {
                uses.add("axum::extract", &["Path"]);
            }
            if self.handlers.iter().any(|handler| handler.status) {
                uses.add("axum::http", &["StatusCode"]);
            }
            let methods: Vec<&str> = self
                .handlers
                .iter()
                .map(|handler| trigger::method_fn(&handler.route.method))
                .collect();
            uses.add("axum::routing", &methods);
        }
        if consuming {
            uses.add("common", &["topic"]);
        }
        if routed || consuming {
            uses.add("omnia_sdk::api", &["Context", "Handlers"]);
        }
        let mut sdk = Vec::new();
        if routed || consuming {
            sdk.push("Capabilities");
        }
        if routed || consuming || !self.functions.is_empty() {
            sdk.push("Result");
        }
        if self.settings.is_some() {
            sdk.push("Settings");
        }
        uses.add("omnia_sdk", &sdk);
        let deserializing = consuming
            || !self.types.is_empty()
            || self.handlers.iter().any(|handler| handler.input.is_some());
        let serializing = routed || !self.types.is_empty();
        let mut serde = Vec::new();
        if deserializing {
            serde.push("Deserialize");
        }
        if serializing {
            serde.push("Serialize");
        }
        uses.add("serde", &serde);
        uses
    }

    // The part's `handlers()`: one `.route(..)` per handler and one
    // `.topic(..)` per consumer, chained over a new `Handlers`.
    fn handlers_fn(&self) -> Fn {
        let calls = self
            .handlers
            .iter()
            .map(Handler::registration)
            .chain(self.consumers.iter().map(Consumer::registration))
            .collect();
        Fn::new("handlers")
            .vis(Vis::Crate)
            .generics("<P: Capabilities>")
            .returns("Handlers<P>")
            .body(Chain::new("Handlers::new()", calls).tail(0))
    }

    fn sending(&self) -> bool {
        self.scenarios.iter().any(|test| matches!(test.act, Act::Route(_)))
    }

    fn delivering(&self) -> bool {
        self.scenarios.iter().any(|test| matches!(test.act, Act::Topic(_)))
    }

    // Extends the held `text` of a module for a later serial slice of the
    // part: inserts missing imports, registers new routes and topics in
    // `handlers()` where `syn` locates its chain (else records them as
    // unanchored), and appends the slice's new path constants, settings,
    // design types, handlers, consumers, and functions at the end of the
    // file.
    pub fn extend_module(&self, text: &str) -> (String, Vec<String>) {
        let parsed = syntax::parse(text).ok();
        let mut next = text.to_owned();
        let mut unanchored = Vec::new();
        let calls: Vec<String> = self
            .handlers
            .iter()
            .map(|handler| handler.registration().inline())
            .chain(self.consumers.iter().map(|consumer| consumer.registration().inline()))
            .collect();
        if !calls.is_empty() && self.held_fn("handlers") {
            if let Some(at) =
                parsed.as_ref().and_then(|file| syntax::chain_end(text, file, "handlers"))
            {
                next.insert_str(at, &calls.concat());
            } else {
                for call in calls {
                    unanchored.push(format!("`{call}` in `handlers()` of `{}`", self.module.path));
                }
            }
        }

        let uses = missing_uses(self.imports(), parsed.as_ref());
        insert_uses(&mut next, uses, parsed.as_ref());

        let mut appended = Source::default();
        if !self.paths.is_empty() {
            let paths = self.paths.iter().map(|(constant, path)| {
                Const::new(constant, "&str", format!("\"{}\"", escape(path))).vis(Vis::Crate)
            });
            appended.push(Consts::new(paths));
        }
        if self.registers() && !self.held_fn("handlers") {
            appended.push(self.handlers_fn());
        }
        if let Some(settings) = &self.settings {
            appended.push(settings.render());
        }
        for design_type in &self.types {
            design_type.render(&mut appended);
        }
        for handler in &self.handlers {
            handler.render(&mut appended);
        }
        for consumer in &self.consumers {
            consumer.render(&mut appended);
        }
        for function in &self.functions {
            appended.push(function.render());
        }
        append_source(&mut next, &appended);
        (next, unanchored)
    }

    // Extends the held `text` of a test file for a later serial slice of the
    // part: inserts missing imports and helpers (`router`, `call`), and
    // appends the slice's new ignored scenario tests at the end of the file.
    pub fn extend_tests(&self, text: &str) -> String {
        if self.scenarios.is_empty() {
            return text.to_owned();
        }
        let parsed = syntax::parse(text).ok();
        let mut next = text.to_owned();
        let uses = missing_uses(self.test_imports(), parsed.as_ref());
        insert_uses(&mut next, uses, parsed.as_ref());

        let ident = self.package.replace('-', "_");
        let mut appended = Source::default();
        if !self.held_fn("provider") {
            appended.push(provider(self.settings.as_ref()));
        }
        if (self.sending() || self.delivering()) && !self.held_fn("router") {
            appended.push(router(&ident));
        }
        if self.sending() && !self.held_fn("call") {
            appended.push(call());
        }
        for test in &self.scenarios {
            appended.push(
                test.render(self)
                    .comments(test.steps())
                    .attr("#[tokio::test]")
                    .attr(test.ignored()),
            );
        }
        append_source(&mut next, &appended);
        next
    }

    fn test_imports(&self) -> Uses {
        let sending = self.sending();
        let delivering = self.delivering();
        let with_body = self
            .scenarios
            .iter()
            .any(|test| matches!(&test.act, Act::Route(route) if self.sends_body(route)));
        let mut uses = Uses::default();
        if sending {
            uses.add("axum::body", &["Body"]);
            if with_body {
                uses.add("axum::http::header", &["CONTENT_TYPE"]);
            }
            uses.add("axum::http", &["Request", "StatusCode"]);
        }
        if delivering {
            uses.add("common", &["topic"]);
        }
        if sending || delivering {
            uses.add("omnia_sdk::api", &["Client", "Router"]);
        }
        let mut doubles = vec!["Provider"];
        if self.settings.is_some() {
            doubles.push("MapConfig");
        }
        if sending {
            doubles.push("send");
        }
        if delivering {
            doubles.push("deliver");
        }
        uses.add("omnia_test::guest", &doubles);
        if sending {
            uses.add("serde_json", &["Value"]);
        }
        uses
    }
}

impl Handler {
    // The `.route(..)` call that registers the handler under the method
    // router for its method.
    fn registration(&self) -> Call {
        let service = Call::new(trigger::method_fn(&self.route.method), [self.name.as_str()]);
        Call::new(".route", [Expr::from(self.constant.as_str()), service.into()])
    }

    // Its input where the method carries a body, its output, and the
    // handler under its requirements: the context first, then each
    // extractor the route needs, its body the marker.
    fn render(&self, source: &mut Source) {
        let route = escape(&format!("{} {}", self.route.method, self.route.path));
        if let Some(input) = &self.input {
            source.push(
                Struct::new(input)
                    .docs([format!("The input of `{route}`.")])
                    .attr("#[derive(Debug, Deserialize)]")
                    .vis(Vis::Pub),
            );
        }
        source.push(
            Struct::new(&self.output)
                .docs([format!("The output of `{route}`.")])
                .attr("#[derive(Debug, Serialize)]")
                .vis(Vis::Pub),
        );
        let mut params = vec!["context: Context<P>".to_owned()];
        match self.params.as_slice() {
            [] => {}
            [one] => params.push(format!("Path({}): Path<String>", one.field)),
            several => {
                let fields: Vec<&str> = several.iter().map(|param| param.field.as_str()).collect();
                let types = vec!["String"; several.len()];
                params.push(format!("Path(({})): Path<({})>", fields.join(", "), types.join(", ")));
            }
        }
        if let Some(input) = &self.input {
            params.push(format!("Json(input): Json<{input}>"));
        }
        let answered = if self.status {
            format!("(StatusCode, Json<{}>)", self.output)
        } else {
            format!("Json<{}>", self.output)
        };
        source.push(
            Fn::new(&self.name)
                .comments(stated(&self.stated))
                .vis(Vis::Crate)
                .asynchronous()
                .generics("<P>")
                .params(params)
                .returns(format!("Result<{answered}>"))
                .bounds([BOUND])
                .body(stub_lines(&self.borrowed(), &self.marker)),
        );
    }
}

impl Consumer {
    fn registration(&self) -> Call {
        Call::new(".topic", [format!("topic::{}", self.constant), self.name.clone()])
    }

    // Its input over the delivery, and the consumer under its requirements,
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
                .generics("<P>")
                .params(["context: Context<P>".to_owned(), format!("input: {}", self.input)])
                .returns("Result<()>")
                .bounds([BOUND])
                .body(stub_lines(&["context".to_owned(), "input".to_owned()], &self.marker)),
        );
    }
}

impl Function {
    // The function from its signature to its closing brace, the comment and
    // doc lines above it standing: the one block the turn replaces, since
    // the behaviour decides the signature too.
    pub fn block(&self) -> String {
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
        Fn::new(&self.name).vis(Vis::Pub).returns("Result<()>").body(marker_lines(&self.marker))
    }
}

impl Settings {
    // The settings struct under the keys it reads: one `String` field per
    // key, under the shared prefix where the keys have one, a key spelled
    // on its field where the derive would not read it off the name.
    fn render(&self) -> Struct {
        let keys: Vec<String> = self.keys().map(|key| format!("`{key}`")).collect();
        let fields = self.fields.iter().map(|field| {
            Field::new(&field.name, "String").attrs(
                field.explicit.then(|| format!("#[setting(key = \"{}\")]", escape(&field.key))),
            )
        });
        Struct::new(&self.name)
            .comments([format!("The settings the part reads: {}.", keys.join(", "))])
            .attr("#[derive(Debug, Settings)]")
            .attrs(self.prefix.as_ref().map(|prefix| format!("#[setting(prefix = \"{prefix}\")]")))
            .vis(Vis::Crate)
            .fields(fields)
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
    pub fn block(&self, stem: &Stem) -> String {
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
                if stem.sends_body(route) {
                    calls.push(Call::new(".header", ["CONTENT_TYPE", "\"application/json\""]));
                    calls.push(Call::new(".body", ["Body::from(\"{}\")"]));
                } else {
                    calls.push(Call::new(".body", ["Body::empty()"]));
                }
                calls.push(Call::new(".expect", ["\"request\""]));
                let receiver =
                    Call::new(format!("Request::{}", trigger::method_fn(&route.method)), [target]);
                lines.extend(Chain::new(receiver, calls).bound(0, "request"));
                lines.push("let (status, body) = call(&provider(), request).await;".to_owned());
                lines.push("let _ = (&status, &body);".to_owned());
                lines
            }
            Act::Topic(topic) => {
                let constant = stem
                    .consumers
                    .iter()
                    .find(|consumer| consumer.topic == *topic)
                    .map_or("TOPIC", |consumer| consumer.constant.as_str());
                let delivery = Call::new(
                    "deliver",
                    [
                        "&router(&provider)".to_owned(),
                        format!("topic::{constant}"),
                        "b\"{}\"".to_owned(),
                    ],
                );
                let mut lines = vec!["let provider = provider();".to_owned()];
                lines.extend(Chain::new(delivery, Vec::new()).awaited().bound(0, "outcome"));
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

// The doubles every scenario starts from, each setting the part reads
// seeded with a placeholder the turn fills.
fn provider(settings: Option<&Settings>) -> Fn {
    let item = Fn::new("provider").returns("Provider");
    let Some(settings) = settings else {
        return item
            .comments([
                "The doubles every scenario starts from: set a setting or a response on them in \
                 the arrangement.",
            ])
            .body(["Provider::default()"]);
    };
    let seeds: Vec<String> =
        settings.keys().map(|key| format!("(\"{}\", \"{PLACEHOLDER}\")", escape(key))).collect();
    let inline = format!("[{}]", seeds.join(", "));
    let joined = format!("Provider::default().config(MapConfig::default().with({inline}))");
    let body = if joined.chars().count() <= BODY_WIDTH {
        vec![joined]
    } else {
        let mut lines = seeded(&seeds, &inline);
        lines.push("Provider::default().config(MapConfig::default().with(settings))".to_owned());
        lines
    };
    item.comments([
        "The doubles every scenario starts from, the part's settings seeded: give each its",
        "value for the scenarios, and set a response on the doubles in the arrangement.",
    ])
    .body(body)
}

// `let settings = [..];` as the formatter lays an array: on one line where
// it fits, beneath the binding where it fits there, else one seed per line.
fn seeded(seeds: &[String], inline: &str) -> Vec<String> {
    let joined = format!("let settings = {inline};");
    if joined.chars().count() <= BODY_WIDTH {
        return vec![joined];
    }
    if TAB + inline.chars().count() < BODY_WIDTH {
        return vec!["let settings =".to_owned(), format!("{}{inline};", pad(TAB))];
    }
    let mut lines = vec!["let settings = [".to_owned()];
    lines.extend(seeds.iter().map(|seed| format!("{}{seed},", pad(TAB))));
    lines.push("];".to_owned());
    lines
}

// The stem's router over the doubles, built as the guest builds it.
fn router(ident: &str) -> Fn {
    Fn::new("router")
        .comments(["The stem's router over the doubles, as the guest builds it."])
        .params(["provider: &Provider"])
        .returns("Router<Provider>")
        .body([format!("{ident}::handlers().with_state(Client::new(provider.clone()))")])
}

// One request through the stem's router, its body read as JSON so a test
// asserts fields and never bytes.
fn call() -> Fn {
    Fn::new("call")
        .comments([
            "One request through the stem's router, answered as its status and its body as JSON:",
            "`Value::Null` for an empty body, the text as a `Value::String` where it is not JSON.",
        ])
        .asynchronous()
        .params(["provider: &Provider", "request: Request<Body>"])
        .returns("(StatusCode, Value)")
        .body([
            "let response = send(&router(provider), request).await;",
            "let status = response.status();",
            "let body = response.into_body();",
            "let body = if body.is_empty() {",
            "    Value::Null",
            "} else {",
            "    serde_json::from_slice(&body)",
            "        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()))",
            "};",
            "(status, body)",
        ])
}

// The body of a handler or a consumer as the file holds it: the borrow of
// `names` that keeps the lints quiet and the marker, the one block the
// turn replaces.
pub fn stub(names: &[String], marker: &str) -> String {
    block(stub_lines(names, marker)).collect::<Vec<_>>().join("\n")
}

fn stub_lines(names: &[String], marker: &str) -> Vec<String> {
    let borrowed: Vec<String> = names.iter().map(|name| format!("&{name}")).collect();
    let borrow = match borrowed.as_slice() {
        [one] => format!("let _ = {one};"),
        several => format!("let _ = ({});", several.join(", ")),
    };
    let mut lines = vec![borrow];
    lines.extend(marker_lines(marker));
    lines
}

// The body of a function as scaffolded: its marker alone.
fn marker_lines(marker: &str) -> Vec<String> {
    Expr::Macro(Call::new("todo!", [format!("\"{}\"", escape(marker))])).tail(0)
}

// `text` in a fence longer than any run of backticks it holds.
pub fn fenced(lang: &str, text: &str) -> String {
    let fence = Fence::around(text);
    format!("{fence}{lang}\n{}\n{fence}\n", text.trim_end_matches('\n'))
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

// `// <id> <subject>: <statement>`, one per requirement an item stands for.
fn stated(stated: &[Stated]) -> Vec<String> {
    stated
        .iter()
        .map(|stated| format!("{} {}: {}", stated.id, stated.subject, stated.statement))
        .collect()
}

// The imports of `uses` that `file` does not already bring into scope.
fn missing_uses(mut uses: Uses, file: Option<&syn::File>) -> Uses {
    if let Some(mentions) = file.map(syntax::Mentions::of) {
        uses.retain(|path, name| {
            let ident = name.split_whitespace().next().unwrap_or(name);
            let tail = [path.rsplit("::").next().unwrap_or(path), ident];
            !(mentions.path_ends(&tail) || mentions.contains(ident))
        });
    }
    uses
}

fn insert_uses(text: &mut String, uses: Uses, file: Option<&syn::File>) {
    if uses.is_empty() {
        return;
    }
    let mut source = Source::default();
    source.push(uses);
    let rendered = source.render();
    let trimmed = rendered.trim_end();
    let mut lines: Vec<&str> = text.lines().collect();
    if let Some(last) = file
        .and_then(syntax::last_use_line)
        .and_then(|line| line.checked_sub(1))
        .or_else(|| lines.iter().rposition(|line| line.starts_with("use ")))
    {
        lines.insert(last + 1, trimmed);
        *text = lines.join("\n") + "\n";
        return;
    }
    if let Some(last) = lines.iter().rposition(|line| line.starts_with("//!")) {
        lines.insert(last + 1, "");
        lines.insert(last + 2, trimmed);
        *text = lines.join("\n") + "\n";
        return;
    }
    *text = format!("{rendered}\n{text}");
}

fn append_source(text: &mut String, appended: &Source) {
    let rendered = appended.render();
    if rendered.is_empty() {
        return;
    }
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        *text = rendered;
    } else {
        *text = format!("{trimmed}\n\n{rendered}");
    }
}

//! Renders the files a scaffold writes and places the lines it adds to the lists.

use std::fmt::Write as _;

use super::names::{escape, flat};
use super::{Consumer, Function, Handler, Stem, Type, route};

// The template's `rustfmt.toml` width, so a scaffolded line the formatter
// would break is written broken.
const MAX_WIDTH: usize = 100;
const PATH_LIST_ANCHOR: &str = "common = { path = \"crates/common\" }";
const ROUTES_ANCHOR: &str = "// one entry per part with routes, each added at the end";
const TOPICS_ANCHOR: &str = "// one entry per part with topics, each added at the end";

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
    // After the last `pub mod` line of a stem's entry.
    AfterLastPubMod,
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
        Place::AfterLastPubMod => {
            lines.iter().rposition(|held| held.trim_start().starts_with("pub mod "))? + 1
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
    text
}

pub fn tests(stem: &Stem) -> String {
    let mut text = format!(
        "//! The acceptance scenarios of the `{}` part of the `{}` stem, over the guest doubles.\n",
        stem.part, stem.name
    );
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
        let _ = writeln!(
            text,
            "#[tokio::test]\n#[ignore = \"{}\"]\nasync fn {}() {{\n    todo!(\"{}\")\n}}",
            escape(&test.ignore),
            test.name,
            escape(&test.marker)
        );
    }
    text
}

fn imports(stem: &Stem) -> String {
    let routed = !stem.handlers.is_empty();
    let consuming = !stem.consumers.is_empty();
    let mut text = String::new();
    if routed || consuming {
        let common = if consuming { "{Capabilities, config, topic}" } else { "Capabilities" };
        let _ = writeln!(text, "use common::{common};");
    }
    if routed {
        let mut methods: Vec<&str> =
            stem.handlers.iter().map(|handler| route::function(&handler.route.method)).collect();
        methods.sort_unstable();
        methods.dedup();
        let methods = if let [method] = methods.as_slice() {
            (*method).to_owned()
        } else {
            format!("{{{}}}", methods.join(", "))
        };
        let _ = writeln!(text, "use omnia_sdk::api::http::{methods};");
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
    let routes: Vec<String> = stem
        .handlers
        .iter()
        .map(|handler| {
            format!(
                ".route({}, {}({}))",
                handler.constant,
                route::function(&handler.route.method),
                handler.name
            )
        })
        .collect();
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
    let _ = writeln!(
        text,
        "/// {fence}\n#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct {} {{}}",
        design_type.name
    );
    text
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
    for stated in &handler.stated {
        let _ = writeln!(text, "// {} {}: {}", stated.id, stated.subject, stated.statement);
    }
    text.push_str(&signature(handler));
    let _ = writeln!(
        text,
        "    let _ = (input, context);\n    todo!(\"{}\")\n}}",
        escape(&handler.marker)
    );
    text
}

fn signature(handler: &Handler) -> String {
    head(&handler.name, &handler.input, &handler.output)
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
    let _ = writeln!(
        text,
        "    let _ = (input, context);\n    todo!(\"{}\")\n}}",
        escape(&consumer.marker)
    );
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
         requirement states.\npub fn {}() -> omnia_sdk::Result<()> {{\n    todo!(\"{}\")\n}}\n",
        function.stated.id,
        function.stated.subject,
        function.stated.statement,
        function.stated.subject,
        function.name,
        escape(&function.marker)
    )
}

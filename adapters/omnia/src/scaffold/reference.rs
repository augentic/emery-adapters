//! Selects, from the embedded references, the passages the slice's shapes compile and test against.

use std::collections::BTreeSet;

use emery_sdk::{Doc, body};

const API: &str = "references/omnia-api.md";
const TESTING: &str = "references/testing.md";
const TRANSLATION: &str = "references/translation.md";

// What the design's `## APIs and integrations` names the program reaching,
// read by the words of its text.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Role {
    Consume,
    Publish,
    Upstream,
    Identity,
    State,
    Blob,
    Document,
    Table,
    Broadcast,
    Monitor,
    Schedule,
    Delay,
}

const ROLE_WORDS: &[(Role, &[&str])] = &[
    (
        Role::Consume,
        &[
            "consume",
            "consumed",
            "consumer",
            "consumers",
            "consumes",
            "inbound",
            "subscribe",
            "subscribed",
            "subscribes",
            "subscription",
        ],
    ),
    (
        Role::Publish,
        &[
            "broker",
            "emit",
            "emits",
            "emitted",
            "eventhub",
            "kafka",
            "nats",
            "outbound",
            "produce",
            "produced",
            "producer",
            "produces",
            "publish",
            "published",
            "publisher",
            "publishes",
            "pubsub",
            "queue",
            "rabbitmq",
            "sns",
            "sqs",
            "topic",
            "topics",
        ],
    ),
    (
        Role::Upstream,
        &[
            "api",
            "apis",
            "endpoint",
            "endpoints",
            "fetch",
            "fetched",
            "fetches",
            "graphql",
            "http",
            "https",
            "rest",
            "upstream",
            "url",
            "urls",
        ],
    ),
    (
        Role::Identity,
        &[
            "bearer",
            "credential",
            "credentials",
            "identity",
            "keyvault",
            "oauth",
            "secret",
            "secrets",
            "token",
            "tokens",
            "vault",
        ],
    ),
    (Role::State, &["cache", "cached", "caches", "caching", "memcached", "redis", "ttl"]),
    (Role::Blob, &["blob", "blobs", "bucket", "buckets", "gcs", "s3"]),
    (
        Role::Document,
        &[
            "collection",
            "collections",
            "cosmos",
            "document",
            "documents",
            "dynamodb",
            "firestore",
            "mongo",
            "mongodb",
        ],
    ),
    (
        Role::Table,
        &[
            "database",
            "databases",
            "mssql",
            "mysql",
            "oracle",
            "postgres",
            "postgresql",
            "relational",
            "sql",
            "sqlite",
        ],
    ),
    (Role::Broadcast, &["socket", "sockets", "websocket", "websockets", "ws"]),
    (
        Role::Monitor,
        &[
            "apm",
            "counter",
            "counters",
            "datadog",
            "elastic",
            "metric",
            "metrics",
            "monitor",
            "monitoring",
            "newrelic",
            "prometheus",
            "span",
            "spans",
            "telemetry",
        ],
    ),
    (
        Role::Schedule,
        &[
            "cron",
            "interval",
            "intervals",
            "periodic",
            "periodically",
            "refresh",
            "refreshed",
            "refreshes",
            "reload",
            "reloaded",
            "reloads",
            "schedule",
            "scheduled",
        ],
    ),
    (
        Role::Delay,
        &["attempts", "backoff", "delay", "delays", "retried", "retries", "retry", "sleep"],
    ),
];

// The translation table's row for each role, by the words its first cell
// opens with.
const TRANSLATION_ROWS: &[(Role, &str)] = &[
    (Role::Consume, "a message is consumed"),
    (Role::Consume, "a consumer's error boundary"),
    (Role::Publish, "a message is published"),
    (Role::Publish, "the broker's addresses"),
    (Role::Consume, "the broker's addresses"),
    (Role::Identity, "a secret read from a vault"),
    (Role::Upstream, "an upstream REST or GraphQL call"),
    (Role::Schedule, "a reference table or a mapping"),
    (Role::State, "a store: a SQL database"),
    (Role::Blob, "a store: a SQL database"),
    (Role::Document, "a store: a SQL database"),
    (Role::Table, "a store: a SQL database"),
    (Role::Delay, "a delay or a sleep"),
    (Role::Monitor, "an APM agent"),
    (Role::Broadcast, "a WebSocket push"),
];

// The capability trait, the double, and the testing table's row each role
// reaches, by the lead of each block.
const CAPABILITIES: &[(Role, &str, &str, &str)] = &[
    (Role::Publish, "pub trait Publish", "impl Sink", "`Publish`"),
    (Role::Upstream, "pub trait HttpRequest", "impl MatchedHttp", "`HttpRequest`"),
    (Role::Identity, "pub trait Identity", "impl FixedIdentity", "`Identity`"),
    (Role::State, "pub trait StateStore", "impl Memory", "`StateStore`"),
    (Role::Schedule, "pub trait StateStore", "impl Memory", "`StateStore`"),
    (Role::Blob, "pub trait BlobStore", "impl Memory", "`StateStore`"),
    (Role::Document, "pub trait DocumentStore", "impl MemoryDocs", "`DocumentStore`"),
    (Role::Table, "pub trait TableStore", "impl ScriptedTables", "`TableStore`"),
    (Role::Broadcast, "pub trait Broadcast", "impl Sink", "`Broadcast`"),
];

// What the scaffold laid, which decides the passages.
pub(super) struct Needs {
    pub shapes: BTreeSet<Shape>,
    pub roles: BTreeSet<Role>,
}

// A kind of item the scaffold laid for the slice.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Shape {
    Routes,
    // A route function over `handle_with`, which spells the encoder.
    Encoded,
    Consumers,
    Tests,
    Types,
}

// One passage of a reference: Markdown quoted verbatim, or code the brief
// fences.
pub(super) struct Passage {
    pub source: &'static str,
    pub heading: String,
    pub text: String,
    pub code: bool,
}

// The roles the design's APIs section names, by the words of its text; none
// where the design has no such section.
pub(super) fn roles(design: &str) -> BTreeSet<Role> {
    let Some((_, section)) = section(design, "APIs and integrations") else {
        return BTreeSet::new();
    };
    let words: BTreeSet<String> = section
        .lines()
        .flat_map(|line| line.split(|ch: char| !ch.is_ascii_alphanumeric()))
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    ROLE_WORDS
        .iter()
        .filter(|(_, spelled)| spelled.iter().any(|word| words.contains(*word)))
        .map(|(role, _)| *role)
        .collect()
}

pub(super) fn passages(docs: &[Doc], needs: &Needs) -> Vec<Passage> {
    let mut passages = Vec::new();
    let has = |shape: Shape| needs.shapes.contains(&shape);
    let mut roles = needs.roles.clone();
    if has(Shape::Consumers) {
        roles.insert(Role::Consume);
    }
    let api = body(docs, API).unwrap_or_default();
    let reached = |lead: fn(&(Role, &'static str, &'static str, &'static str)) -> &'static str| {
        CAPABILITIES.iter().filter(|(role, ..)| roles.contains(role)).map(lead).collect::<Vec<_>>()
    };

    // the routing and the handler's context
    if has(Shape::Routes)
        && let Some((heading, routing)) = section(api, "Routing")
    {
        if let Some(plain) = before(routing, "The general constructor") {
            passages.push(markdown(API, &heading, plain));
        }
        if has(Shape::Encoded)
            && let Some(general) =
                between(routing, "The general constructor", "The router is axum's")
        {
            passages.push(markdown(API, &heading, general));
        }
    }
    if has(Shape::Consumers)
        && let Some((heading, messaging)) = section(api, "Messaging")
    {
        passages.push(markdown(API, &heading, messaging));
    }
    if (has(Shape::Routes) || has(Shape::Consumers))
        && let Some((heading, handler)) = section(api, "The handler")
        && let Some(context) =
            fence(handler, 0).and_then(|fence| paragraph(fence, "pub struct Context<P>"))
    {
        passages.push(code(API, &heading, context));
    }
    if let Some((heading, errors)) = section(api, "Errors") {
        passages.push(markdown(API, &heading, errors));
    }

    // the capabilities the roles reach, `Config` always
    if let Some((heading, capabilities)) = section(api, "The capabilities")
        && let Some(fence) = fence(capabilities, 0)
    {
        let leads = [vec!["pub trait Config"], reached(|(_, lead, ..)| lead)].concat();
        passages.push(code(API, &heading, &paragraphs(fence, &leads)));
    }

    // the doubles the tests arrange, `Provider` and `MapConfig` always
    if has(Shape::Tests) {
        if let Some((heading, doubles)) = section(api, "The doubles")
            && let Some(fence) = fence(doubles, 0)
        {
            let seeding = reached(|(_, _, lead, _)| lead);
            let leads = [vec!["pub struct Provider", "impl MapConfig"], seeding].concat();
            passages.push(code(API, &heading, &paragraphs(fence, &leads)));
        }
        if let Some(testing) = body(docs, TESTING)
            && let Some((heading, doubles)) = section(testing, "The doubles")
        {
            let cells = [vec!["`Config`"], reached(|(.., cell)| cell)].concat();
            let rows = table(doubles, |first| cells.iter().any(|cell| first.contains(cell)));
            if !rows.is_empty() {
                passages.push(markdown(TESTING, &heading, &rows));
            }
        }
    }

    // the translation of each role, and of the design types the slice owns
    if let Some(translation) = body(docs, TRANSLATION) {
        if let Some((heading, table_section)) = section(translation, "The table") {
            let mut leads: Vec<&str> = TRANSLATION_ROWS
                .iter()
                .filter(|(role, _)| roles.contains(role))
                .map(|(_, lead)| *lead)
                .collect();
            if has(Shape::Types) {
                leads.push("a design type");
            }
            let rows =
                table(table_section, |first| leads.iter().any(|lead| first.starts_with(lead)));
            if !rows.is_empty() {
                passages.push(markdown(TRANSLATION, &heading, &rows));
            }
        }
        if roles.contains(&Role::Schedule)
            && let Some((heading, fragment)) = section(translation, "A reference table")
        {
            passages.push(markdown(TRANSLATION, &heading, fragment));
        }
    }
    passages
}

// The paragraphs of `text` each lead opens, once each, in the leads' order.
fn paragraphs(text: &str, leads: &[&str]) -> String {
    let mut blocks: Vec<&str> = Vec::new();
    for lead in leads {
        if let Some(block) = paragraph(text, lead)
            && !blocks.contains(&block)
        {
            blocks.push(block);
        }
    }
    blocks.join("\n\n")
}

fn markdown(source: &'static str, heading: &str, text: &str) -> Passage {
    Passage {
        source,
        heading: heading.to_owned(),
        text: text.trim().to_owned(),
        code: false,
    }
}

fn code(source: &'static str, heading: &str, text: &str) -> Passage {
    Passage {
        source,
        heading: heading.to_owned(),
        text: text.trim_end().to_owned(),
        code: true,
    }
}

// The section whose `##` heading starts with `heading`: the heading's text
// and the body below it, up to the next heading of the same or a higher
// level outside a fence.
fn section<'t>(text: &'t str, heading: &str) -> Option<(String, &'t str)> {
    let opened = format!("## {heading}");
    let mut in_fence = false;
    let mut start = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence {
            match start {
                None if line.starts_with(&opened) => start = Some((at, line)),
                Some((from, title)) if line.starts_with("## ") || line.starts_with("# ") => {
                    return Some((title_of(title), text[from + title.len()..at].trim()));
                }
                _ => {}
            }
        }
        at += line.len();
    }
    start.map(|(from, title)| (title_of(title), text[from + title.len()..].trim()))
}

fn title_of(line: &str) -> String {
    line.trim_start_matches('#').trim().to_owned()
}

// The section up to the line that starts with `stop`, exclusive.
fn before<'t>(section: &'t str, stop: &str) -> Option<&'t str> {
    let at = line_start(section, stop)?;
    Some(section[..at].trim_end())
}

// The section from the line that starts with `from` to the line that starts
// with `to`, exclusive.
fn between<'t>(section: &'t str, from: &str, to: &str) -> Option<&'t str> {
    let start = line_start(section, from)?;
    let end = line_start(&section[start..], to).map_or(section.len(), |at| start + at);
    Some(section[start..end].trim_end())
}

// The byte offset of the first line that starts with `lead`.
fn line_start(text: &str, lead: &str) -> Option<usize> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with(lead) {
            return Some(at);
        }
        at += line.len();
    }
    None
}

// The content of the `n`th fenced block of the section.
fn fence(section: &str, n: usize) -> Option<&str> {
    let mut blocks = Vec::new();
    let mut open: Option<usize> = None;
    let mut at = 0;
    for line in section.split_inclusive('\n') {
        if line.starts_with("```") {
            match open.take() {
                Some(start) => blocks.push(&section[start..at]),
                None => open = Some(at + line.len()),
            }
        }
        at += line.len();
    }
    blocks.get(n).copied()
}

// The lines from the first that starts with `lead` to the next blank line,
// exclusive.
fn paragraph<'t>(text: &'t str, lead: &str) -> Option<&'t str> {
    let start = line_start(text, lead)?;
    let rest = &text[start..];
    let end = rest.find("\n\n").unwrap_or(rest.len());
    Some(rest[..end].trim_end())
}

// The table's header rows and every body row whose first cell `keep` admits.
fn table(section: &str, keep: impl Fn(&str) -> bool) -> String {
    let mut rows: Vec<&str> = Vec::new();
    let mut header = 0;
    for line in section.lines().filter(|line| line.starts_with('|')) {
        let first = line.trim_start_matches('|').split('|').next().unwrap_or_default().trim();
        if header < 2 {
            rows.push(line);
            header += 1;
        } else if keep(first) {
            rows.push(line);
        }
    }
    if rows.len() <= 2 {
        return String::new();
    }
    rows.join("\n")
}

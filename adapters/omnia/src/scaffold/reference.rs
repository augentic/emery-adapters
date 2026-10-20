//! Selects, from the embedded references, the passages the items the scaffold laid compile and test against.

use std::collections::BTreeSet;
use std::iter;

use emery_sdk::target::Slice;
use emery_sdk::{Doc, body, push_unique, words};

const API: &str = "references/omnia-api.md";
const HANDLERS: &str = "references/handlers.md";
const TESTING: &str = "references/testing.md";
const TRANSLATION: &str = "references/translation.md";

// What the slice's text and the design's `## APIs and integrations` name
// the program reaching, read by the words of their text.
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
    // `api`, `http`, and `rest` name the inbound surface as often as an
    // upstream, and a `url` names a store's connection as often, so none of
    // them selects the role.
    (
        Role::Upstream,
        &["endpoint", "endpoints", "fetch", "fetched", "fetches", "graphql", "upstream"],
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

// An idiom of the handlers reference the slice's own text calls for, by
// the words of its requirements and brief alone, since the design's words
// name what every slice shares.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Idiom {
    Probe,
    CacheAside,
}

// Each idiom with the heading of its section and the words that select it.
const IDIOMS: &[(Idiom, &str, &[&str])] = &[
    (
        Idiom::Probe,
        "A probe",
        &["deadline", "deadlines", "probe", "probed", "probes", "readiness", "timeout", "timeouts"],
    ),
    (
        Idiom::CacheAside,
        "A cache-aside read",
        &[
            "cache",
            "cached",
            "caches",
            "caching",
            "invalidate",
            "invalidated",
            "invalidates",
            "invalidating",
            "invalidation",
            "ttl",
        ],
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

// A role with the capability trait, the double, and the testing table's row
// it reaches, by the lead of each block.
type Capability = (Role, &'static str, &'static str, &'static str);

const CAPABILITIES: &[Capability] = &[
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

// What the scaffold laid and the slice reaches, which decide the passages.
pub(super) struct Needs {
    pub laid: BTreeSet<Laid>,
    pub roles: BTreeSet<Role>,
    pub idioms: BTreeSet<Idiom>,
}

// A kind of item the scaffold laid for the slice.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Laid {
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

// The roles the slice reaches, by the words of its own text and of the
// design's APIs paragraphs that speak to it: a paragraph citing one of the
// slice's requirement ids, or citing none, names what the slice reaches; one
// citing other slices' ids alone names what they reach.
pub(super) fn roles(slice: &Slice) -> BTreeSet<Role> {
    let ids: Vec<&str> =
        slice.requirements.iter().map(|requirement| requirement.id.as_str()).collect();
    let mut spelled = own_words(slice);
    if let Some((_, section)) = section(&slice.design, "APIs and integrations") {
        for paragraph in section.split("\n\n") {
            let cited = cited(paragraph);
            if cited.is_empty() || cited.iter().any(|id| ids.contains(&id.as_str())) {
                spell(paragraph, &mut spelled);
            }
        }
    }
    ROLE_WORDS
        .iter()
        .filter(|(_, words)| words.iter().any(|word| spelled.contains(*word)))
        .map(|(role, _)| *role)
        .collect()
}

// The idioms the slice's own text calls for.
pub(super) fn idioms(slice: &Slice) -> BTreeSet<Idiom> {
    let spelled = own_words(slice);
    IDIOMS
        .iter()
        .filter(|(_, _, words)| words.iter().any(|word| spelled.contains(*word)))
        .map(|(idiom, ..)| *idiom)
        .collect()
}

// Every word the slice's own text spells: its requirements, their
// scenarios, and its brief.
fn own_words(slice: &Slice) -> BTreeSet<String> {
    let mut spelled = BTreeSet::new();
    for requirement in &slice.requirements {
        spell(&requirement.subject, &mut spelled);
        for paragraph in &requirement.statement {
            spell(paragraph, &mut spelled);
        }
        for scenario in &requirement.scenarios {
            let lines = iter::once(&scenario.name)
                .chain(&scenario.given)
                .chain([&scenario.when, &scenario.then])
                .chain(&scenario.and);
            for line in lines {
                spell(line, &mut spelled);
            }
        }
    }
    for paragraph in &slice.brief {
        spell(paragraph, &mut spelled);
    }
    spelled
}

// Adds every word `text` spells, lowercased: each run of ASCII
// alphanumerics whole, and each word the lexicon's splitter cuts at a
// camel-case boundary within one, so `PostgreSQL` spells `postgresql` and
// `sql` both.
fn spell(text: &str, into: &mut BTreeSet<String>) {
    into.extend(
        text.split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_ascii_lowercase),
    );
    into.extend(words(text));
}

// The requirement ids the paragraph cites, a `REQ-a through REQ-b` range
// expanded to every id within it.
fn cited(paragraph: &str) -> Vec<String> {
    const ID: &str = "REQ-";
    const THROUGH: &str = " through REQ-";
    let mut ids = Vec::new();
    let mut rest = paragraph;
    while let Some(at) = rest.find(ID) {
        let (first, after) = digits(&rest[at + ID.len()..]);
        rest = after;
        if first.is_empty() {
            continue;
        }
        ids.push(format!("{ID}{first}"));
        if let Some(after) = after.strip_prefix(THROUGH) {
            let (last, after) = digits(after);
            if let (Ok(from), Ok(to)) = (first.parse::<u32>(), last.parse::<u32>()) {
                ids.extend((from..=to).map(|n| format!("{ID}{n:0width$}", width = first.len())));
                rest = after;
            }
        }
    }
    ids
}

// The leading run of ASCII digits and the text after it.
fn digits(text: &str) -> (&str, &str) {
    let rest = text.trim_start_matches(|ch: char| ch.is_ascii_digit());
    text.split_at(text.len() - rest.len())
}

pub(super) fn passages(docs: &[Doc], needs: &Needs) -> Vec<Passage> {
    let mut passages = Vec::new();
    let has = |laid: Laid| needs.laid.contains(&laid);
    let mut roles = needs.roles.clone();
    if has(Laid::Consumers) {
        roles.insert(Role::Consume);
    }
    let api = body(docs, API).unwrap_or_default();

    // the routing and the handler's context
    if has(Laid::Routes)
        && let Some((heading, routing)) = section(api, "Routing")
    {
        if let Some(plain) = before(routing, "The general constructor") {
            passages.push(markdown(API, &heading, plain));
        }
        if has(Laid::Encoded)
            && let Some(general) =
                between(routing, "The general constructor", "The router is axum's")
        {
            passages.push(markdown(API, &heading, general));
        }
    }
    if has(Laid::Consumers)
        && let Some((heading, messaging)) = section(api, "Messaging")
    {
        passages.push(markdown(API, &heading, messaging));
    }
    if (has(Laid::Routes) || has(Laid::Consumers))
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
        let leads = iter::once("pub trait Config").chain(reached(&roles, |(_, lead, ..)| lead));
        passages.push(code(API, &heading, &paragraphs(fence, leads)));
    }

    // the idioms the slice's text calls for, each its section whole
    if let Some(handlers) = body(docs, HANDLERS) {
        for (idiom, lead, _) in IDIOMS {
            if needs.idioms.contains(idiom)
                && let Some((heading, idiom)) = section(handlers, lead)
            {
                passages.push(markdown(HANDLERS, &heading, idiom));
            }
        }
    }

    // the doubles the tests arrange, `Provider` and `MapConfig` always
    if has(Laid::Tests) {
        if let Some((heading, doubles)) = section(api, "The doubles")
            && let Some(fence) = fence(doubles, 0)
        {
            let leads = ["pub struct Provider", "impl MapConfig"]
                .into_iter()
                .chain(reached(&roles, |(_, _, lead, _)| lead));
            passages.push(code(API, &heading, &paragraphs(fence, leads)));
        }
        if let Some(testing) = body(docs, TESTING)
            && let Some((heading, doubles)) = section(testing, "The doubles")
        {
            let cells: Vec<&str> =
                iter::once("`Config`").chain(reached(&roles, |(.., cell)| cell)).collect();
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
            if has(Laid::Types) {
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

// One lead of each capability block the roles reach, in the table's order.
fn reached(
    roles: &BTreeSet<Role>, lead: fn(&Capability) -> &'static str,
) -> impl Iterator<Item = &'static str> {
    CAPABILITIES.iter().filter(move |(role, ..)| roles.contains(role)).map(lead)
}

// The paragraphs of `text` each lead opens, once each, in the leads' order.
fn paragraphs<'l>(text: &str, leads: impl IntoIterator<Item = &'l str>) -> String {
    let mut blocks: Vec<&str> = Vec::new();
    for lead in leads {
        if let Some(block) = paragraph(text, lead) {
            push_unique(&mut blocks, block);
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

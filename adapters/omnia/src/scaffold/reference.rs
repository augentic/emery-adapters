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
pub enum Role {
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
    // upstream, a `url` names a store's connection as often, and a `fetch`
    // a store's read, so none of them selects the role.
    (Role::Upstream, &["endpoint", "endpoints", "graphql", "upstream"]),
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
pub enum Idiom {
    Probe,
    CacheAside,
    OptimisticLock,
}

// An idiom with the heading of its section and the words that select it.
struct IdiomSpec {
    idiom: Idiom,
    heading: &'static str,
    words: &'static [&'static str],
}

const IDIOMS: &[IdiomSpec] = &[
    IdiomSpec {
        idiom: Idiom::Probe,
        heading: "A probe",
        words: &[
            "deadline",
            "deadlines",
            "probe",
            "probed",
            "probes",
            "readiness",
            "timeout",
            "timeouts",
        ],
    },
    IdiomSpec {
        idiom: Idiom::CacheAside,
        heading: "A cache-aside read",
        words: &[
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
    },
    IdiomSpec {
        idiom: Idiom::OptimisticLock,
        heading: "A compare-and-swap update",
        words: &["cas", "etag", "optimistic"],
    },
];

// The wire format of the inbound stream's payload where the design says it
// is no JSON, the SDK's default decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Xml,
    Csv,
    Protobuf,
    Avro,
}

impl Wire {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Xml => "XML",
            Self::Csv => "CSV",
            Self::Protobuf => "protobuf",
            Self::Avro => "Avro",
        }
    }

    // The clause naming the reader a decoder of the wire goes through,
    // where the seed pins one; empty where the turn chooses.
    pub const fn reader(self) -> &'static str {
        match self {
            Self::Xml => {
                " — `quick_xml::de::from_reader(delivery.payload.as_slice())`, with \
                 `quick-xml.workspace = true` in the stem's `[dependencies]`"
            }
            Self::Csv | Self::Protobuf | Self::Avro => "",
        }
    }
}

const WIRE_WORDS: &[(Wire, &[&str])] = &[
    (Wire::Xml, &["xml"]),
    (Wire::Csv, &["csv"]),
    (Wire::Protobuf, &["protobuf", "proto"]),
    (Wire::Avro, &["avro"]),
];

// The words a sentence spells of a delivery being read, so a format named
// beside one is the inbound payload's and not an export's.
const INBOUND_WORDS: &[&str] = &[
    "consume",
    "consumed",
    "consumer",
    "consumers",
    "consumes",
    "consuming",
    "decode",
    "decoded",
    "decodes",
    "decoding",
    "deliver",
    "delivered",
    "deliveries",
    "delivers",
    "delivery",
    "deserialisation",
    "deserialise",
    "deserialised",
    "deserialises",
    "deserialising",
    "deserialization",
    "deserialize",
    "deserialized",
    "deserializes",
    "deserializing",
    "inbound",
    "incoming",
    "parse",
    "parsed",
    "parses",
    "received",
    "receives",
    "subscribe",
    "subscribed",
    "subscriber",
    "subscribers",
    "subscribes",
    "subscription",
];

// The words a sentence spells of a message leaving, so a format named
// alone in the inbound stream's bullet is not read from its export clause.
const OUTBOUND_WORDS: &[&str] = &[
    "emits",
    "emitted",
    "export",
    "exported",
    "exports",
    "outbound",
    "outgoing",
    "producer",
    "producers",
    "publish",
    "published",
    "publishes",
    "publishing",
    "response",
    "responses",
    "sends",
    "sent",
    "writes",
    "written",
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
// it reaches, by the lead of each block and the row's first cell.
struct Capability {
    role: Role,
    trait_lead: &'static str,
    double_lead: &'static str,
    table_cell: &'static str,
}

impl Capability {
    const fn new(
        role: Role, trait_lead: &'static str, double_lead: &'static str, table_cell: &'static str,
    ) -> Self {
        Self {
            role,
            trait_lead,
            double_lead,
            table_cell,
        }
    }
}

const CAPABILITIES: &[Capability] = &[
    Capability::new(Role::Publish, "pub trait Publish", "impl Sink", "`Publish`"),
    Capability::new(Role::Upstream, "pub trait HttpRequest", "impl MatchedHttp", "`HttpRequest`"),
    Capability::new(Role::Identity, "pub trait Identity", "impl FixedIdentity", "`Identity`"),
    Capability::new(Role::State, "pub trait StateStore", "impl Memory", "`StateStore`"),
    Capability::new(Role::Schedule, "pub trait StateStore", "impl Memory", "`StateStore`"),
    Capability::new(Role::Blob, "pub trait BlobStore", "impl Memory", "`StateStore`"),
    Capability::new(
        Role::Document,
        "pub trait DocumentStore",
        "impl MemoryDocs",
        "`DocumentStore`",
    ),
    Capability::new(Role::Table, "pub trait TableStore", "impl ScriptedTables", "`TableStore`"),
    Capability::new(Role::Broadcast, "pub trait Broadcast", "impl Sink", "`Broadcast`"),
];

// What the scaffold laid and the slice reaches, which decide the passages.
pub struct Needs {
    pub laid: BTreeSet<Laid>,
    pub roles: BTreeSet<Role>,
    pub idioms: BTreeSet<Idiom>,
}

// A kind of item the scaffold laid for the slice.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Laid {
    Routes,
    // A handler answering a status of its own, since a scenario states one.
    Status,
    Consumers,
    // A settings struct over the keys the slice spells.
    Settings,
    Tests,
    Types,
}

// One passage of a reference: Markdown quoted verbatim, or code the brief
// fences.
pub struct Passage {
    pub source: &'static str,
    pub heading: String,
    pub text: String,
    pub code: bool,
}

// The roles the slice reaches, by the words of its own text and of the
// design's APIs paragraphs that speak to it: a paragraph citing one of the
// slice's requirement ids, or citing none, names what the slice reaches; one
// citing other slices' ids alone names what they reach.
pub fn roles(slice: &Slice) -> BTreeSet<Role> {
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

// The wire format of the inbound stream the slice consumes, where the design
// says it is no JSON: the first paragraph naming one decides, read from the
// design's `## APIs and integrations` and `## Technical logic` paragraphs
// that speak to the slice, as `roles` reads them, then from the slice's own
// text. None means JSON, the default decoder's. The wire is the slice's, not
// a topic's: a slice consuming two streams of two formats is told the first
// found.
pub fn wire(slice: &Slice) -> Option<Wire> {
    let ids: Vec<&str> =
        slice.requirements.iter().map(|requirement| requirement.id.as_str()).collect();
    let mut texts: Vec<&str> = Vec::new();
    for heading in ["APIs and integrations", "Technical logic"] {
        if let Some((_, section)) = section(&slice.design, heading) {
            for paragraph in section.split("\n\n") {
                let cited = cited(paragraph);
                if cited.is_empty() || cited.iter().any(|id| ids.contains(&id.as_str())) {
                    texts.push(paragraph);
                }
            }
        }
    }
    texts.extend(own_texts(slice));
    texts.iter().find_map(|text| wire_in(text))
}

// The wire a paragraph names: a sentence spelling a format beside an inbound
// word, or, where the paragraph's lead spells one (the inbound stream's own
// bullet), a sentence spelling the format alone, with no word of a message
// leaving.
fn wire_in(paragraph: &str) -> Option<Wire> {
    let mut sentences = sentences(paragraph).peekable();
    let inbound = sentences.peek().is_some_and(|lead| spells(lead, INBOUND_WORDS));
    sentences.find_map(|sentence| {
        let wire = wire_of(sentence)?;
        (spells(sentence, INBOUND_WORDS) || (inbound && !spells(sentence, OUTBOUND_WORDS)))
            .then_some(wire)
    })
}

// The wire one sentence spells, whatever stands beside it.
fn wire_of(sentence: &str) -> Option<Wire> {
    WIRE_WORDS.iter().find(|(_, words)| spells(sentence, words)).map(|(wire, _)| *wire)
}

fn spells(text: &str, words: &[&str]) -> bool {
    let mut spelled = BTreeSet::new();
    spell(text, &mut spelled);
    words.iter().any(|word| spelled.contains(*word))
}

// The sentences of `text`: each line cut after a full stop, a semicolon, a
// question mark, or an exclamation mark that a space or the line's end
// follows, so a dot within a path or a version continues its sentence.
fn sentences(text: &str) -> impl Iterator<Item = &str> {
    text.lines().flat_map(|line| {
        let mut sentences = Vec::new();
        let mut start = 0;
        let bytes = line.as_bytes();
        for (at, byte) in bytes.iter().enumerate() {
            if matches!(byte, b'.' | b';' | b'?' | b'!')
                && bytes.get(at + 1).is_none_or(u8::is_ascii_whitespace)
            {
                sentences.push(&line[start..=at]);
                start = at + 1;
            }
        }
        if start < line.len() {
            sentences.push(&line[start..]);
        }
        sentences
    })
}

// The idioms the slice's own text calls for.
pub fn idioms(slice: &Slice) -> BTreeSet<Idiom> {
    let spelled = own_words(slice);
    IDIOMS
        .iter()
        .filter(|spec| spec.words.iter().any(|word| spelled.contains(*word)))
        .map(|spec| spec.idiom)
        .collect()
}

// Every word the slice's own text spells.
fn own_words(slice: &Slice) -> BTreeSet<String> {
    let mut spelled = BTreeSet::new();
    for text in own_texts(slice) {
        spell(text, &mut spelled);
    }
    spelled
}

// The slice's own texts: its requirements' subjects and statements, their
// scenarios' lines, and its brief.
fn own_texts(slice: &Slice) -> Vec<&str> {
    let mut texts = Vec::new();
    for requirement in &slice.requirements {
        texts.push(requirement.subject.as_str());
        texts.extend(requirement.statement.iter().map(String::as_str));
        for scenario in &requirement.scenarios {
            texts.push(scenario.name.as_str());
            texts.extend(scenario.given.iter().map(String::as_str));
            texts.push(scenario.when.as_str());
            texts.push(scenario.then.as_str());
            texts.extend(scenario.and.iter().map(String::as_str));
        }
    }
    texts.extend(slice.brief.iter().map(String::as_str));
    texts
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

pub fn passages(docs: &[Doc], needs: &Needs) -> Vec<Passage> {
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
        passages.push(markdown(API, &heading, routing));
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
        let leads = iter::once("pub trait Config")
            .chain(reached(&roles, |capability| capability.trait_lead));
        passages.push(code(API, &heading, &paragraphs(fence, leads)));
    }
    if has(Laid::Settings)
        && let Some((heading, settings)) = section(api, "Settings")
    {
        passages.push(markdown(API, &heading, settings));
    }

    // the idioms the slice's text calls for, each its section whole, and
    // the status shape where a handler answers one
    if let Some(handlers) = body(docs, HANDLERS) {
        for spec in IDIOMS {
            if needs.idioms.contains(&spec.idiom)
                && let Some((heading, idiom)) = section(handlers, spec.heading)
            {
                passages.push(markdown(HANDLERS, &heading, idiom));
            }
        }
        if has(Laid::Status)
            && let Some((heading, status)) = section(handlers, "A status of its own")
        {
            passages.push(markdown(HANDLERS, &heading, status));
        }
    }

    // the doubles the tests arrange, `Provider` and `MapConfig` always, and
    // the calls the prelude makes through them
    if has(Laid::Tests) {
        if let Some((heading, doubles)) = section(api, "The doubles")
            && let Some(fence) = fence(doubles, 0)
        {
            let calls = [
                (has(Laid::Routes), "pub async fn send"),
                (has(Laid::Consumers), "pub async fn deliver"),
            ];
            let leads = ["pub struct Provider", "impl MapConfig"]
                .into_iter()
                .chain(reached(&roles, |capability| capability.double_lead))
                .chain(calls.into_iter().filter(|(laid, _)| *laid).map(|(_, lead)| lead));
            passages.push(code(API, &heading, &paragraphs(fence, leads)));
        }
        if let Some(testing) = body(docs, TESTING)
            && let Some((heading, doubles)) = section(testing, "The doubles")
        {
            let cells: Vec<&str> = iter::once("`Config`")
                .chain(reached(&roles, |capability| capability.table_cell))
                .collect();
            let rows = table(doubles, |first| cells.iter().any(|cell| first.contains(cell)));
            if !rows.is_empty() {
                passages.push(markdown(TESTING, &heading, &rows));
            }
        }
    }

    // the SQL query shape after doubles, so `Laid::Tests` never loses its
    // doubles to the inline budget
    if roles.contains(&Role::Table)
        && let Some(handlers) = body(docs, HANDLERS)
        && let Some((heading, sql_query)) = section(handlers, "A SQL query")
    {
        passages.push(markdown(HANDLERS, &heading, sql_query));
    }

    // the translation of each role, and of the design types the slice owns
    if let Some(translation) = body(docs, TRANSLATION) {
        translation_passages(translation, &roles, has(Laid::Types), &mut passages);
    }
    passages
}

fn translation_passages(
    translation: &str, roles: &BTreeSet<Role>, laid_types: bool, passages: &mut Vec<Passage>,
) {
    if let Some((heading, table_section)) = section(translation, "The table") {
        let mut leads: Vec<&str> = TRANSLATION_ROWS
            .iter()
            .filter(|(role, _)| roles.contains(role))
            .map(|(_, lead)| *lead)
            .collect();
        if laid_types {
            leads.push("a design type");
        }
        let rows = table(table_section, |first| leads.iter().any(|lead| first.starts_with(lead)));
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

// One lead of each capability block the roles reach, in the table's order.
fn reached(
    roles: &BTreeSet<Role>, lead: fn(&Capability) -> &'static str,
) -> impl Iterator<Item = &'static str> {
    CAPABILITIES.iter().filter(move |capability| roles.contains(&capability.role)).map(lead)
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
    let mut count = 0;
    let mut open: Option<usize> = None;
    let mut at = 0;
    for line in section.split_inclusive('\n') {
        if line.starts_with("```") {
            match open.take() {
                Some(start) => {
                    if count == n {
                        return Some(&section[start..at]);
                    }
                    count += 1;
                }
                None => open = Some(at + line.len()),
            }
        }
        at += line.len();
    }
    None
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

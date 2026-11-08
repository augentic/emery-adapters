//! The settings a part reads, as the slice's text spells them: `SCREAMING_SNAKE` keys of two or more segments.

use std::collections::BTreeSet;

use super::names::snake;

// The settings struct a part loads: one field per key the slice spells,
// under the prefix every key shares where one does.
#[derive(Clone)]
pub struct Settings {
    pub name: String,
    pub prefix: Option<String>,
    pub fields: Vec<Field>,
}

// One setting: its field, its key, and whether the key must be spelled on
// the field because the derive would not read it off the field's name.
#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub key: String,
    pub explicit: bool,
}

// The names `axum::http::StatusCode` declares, which a scenario spells for
// a status and never for a setting.
const STATUSES: &[&str] = &[
    "ACCEPTED",
    "ALREADY_REPORTED",
    "BAD_GATEWAY",
    "BAD_REQUEST",
    "CONFLICT",
    "CONTINUE",
    "CREATED",
    "EARLY_HINTS",
    "EXPECTATION_FAILED",
    "FAILED_DEPENDENCY",
    "FORBIDDEN",
    "FOUND",
    "GATEWAY_TIMEOUT",
    "GONE",
    "HTTP_VERSION_NOT_SUPPORTED",
    "IM_A_TEAPOT",
    "IM_USED",
    "INSUFFICIENT_STORAGE",
    "INTERNAL_SERVER_ERROR",
    "LENGTH_REQUIRED",
    "LOCKED",
    "LOOP_DETECTED",
    "METHOD_NOT_ALLOWED",
    "MISDIRECTED_REQUEST",
    "MOVED_PERMANENTLY",
    "MULTI_STATUS",
    "MULTIPLE_CHOICES",
    "NETWORK_AUTHENTICATION_REQUIRED",
    "NO_CONTENT",
    "NON_AUTHORITATIVE_INFORMATION",
    "NOT_ACCEPTABLE",
    "NOT_EXTENDED",
    "NOT_FOUND",
    "NOT_IMPLEMENTED",
    "NOT_MODIFIED",
    "OK",
    "PARTIAL_CONTENT",
    "PAYLOAD_TOO_LARGE",
    "PAYMENT_REQUIRED",
    "PERMANENT_REDIRECT",
    "PRECONDITION_FAILED",
    "PRECONDITION_REQUIRED",
    "PROCESSING",
    "PROXY_AUTHENTICATION_REQUIRED",
    "RANGE_NOT_SATISFIABLE",
    "REQUEST_HEADER_FIELDS_TOO_LARGE",
    "REQUEST_TIMEOUT",
    "RESET_CONTENT",
    "SEE_OTHER",
    "SERVICE_UNAVAILABLE",
    "SWITCHING_PROTOCOLS",
    "TEMPORARY_REDIRECT",
    "TOO_EARLY",
    "TOO_MANY_REQUESTS",
    "UNAUTHORIZED",
    "UNAVAILABLE_FOR_LEGAL_REASONS",
    "UNPROCESSABLE_ENTITY",
    "UNSUPPORTED_MEDIA_TYPE",
    "UPGRADE_REQUIRED",
    "URI_TOO_LONG",
    "USE_PROXY",
    "VARIANT_ALSO_NEGOTIATES",
];

// Names of the shape a key has that a text spells for something else: a
// header, an encoding, a format.
const NOT_SETTINGS: &[&str] =
    &["APPLICATION_JSON", "CONTENT_LENGTH", "CONTENT_TYPE", "ISO_8601", "RFC_3339", "UTF_8"];

impl Settings {
    // The settings `texts` spell for the part `name` stands for, or none
    // where they spell no key: a key is a word of two or more
    // `SCREAMING_SNAKE` segments that opens with a letter and that
    // `excluded` does not claim for a topic, a path, or another constant.
    pub fn spelled<'t>(
        name: String, texts: impl IntoIterator<Item = &'t str>, excluded: impl Fn(&str) -> bool,
    ) -> Option<Self> {
        let keys: BTreeSet<&str> = texts
            .into_iter()
            .flat_map(|text| text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_'))
            .filter(|word| is_key(word) && !excluded(word))
            .collect();
        if keys.is_empty() {
            return None;
        }
        let prefix = shared_prefix(&keys);
        let fields = keys
            .iter()
            .map(|key| {
                let rest =
                    prefix.as_deref().and_then(|prefix| key.strip_prefix(prefix)).unwrap_or(key);
                let field = snake(&rest.to_ascii_lowercase());
                let read =
                    format!("{}{}", prefix.as_deref().unwrap_or(""), field.to_ascii_uppercase());
                Field {
                    explicit: read != *key,
                    name: field,
                    key: (*key).to_owned(),
                }
            })
            .collect();
        Some(Self { name, prefix, fields })
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|field| field.key.as_str())
    }
}

// Whether `word` has a key's shape and is no status.
fn is_key(word: &str) -> bool {
    let mut segments = word.split('_');
    let opens_with_letter = word.starts_with(|ch: char| ch.is_ascii_uppercase());
    let screaming = segments.by_ref().all(|segment| {
        !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    });
    opens_with_letter
        && screaming
        && word.matches('_').count() >= 1
        && !STATUSES.contains(&word)
        && !NOT_SETTINGS.contains(&word)
}

// The first segment every key shares, with its underscore, where every key
// has a segment after it; the derive reads each field under it.
fn shared_prefix(keys: &BTreeSet<&str>) -> Option<String> {
    let mut leads = keys.iter().map(|key| key.split_once('_').map(|(lead, _)| lead));
    let lead = leads.next().flatten()?;
    leads.all(|other| other == Some(lead)).then(|| format!("{lead}_"))
}

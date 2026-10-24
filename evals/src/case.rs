//! Reads a case's `expected.toml` and holds its fixture trees to the credential gate.

use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU32;
use std::path::Path;
use std::{fs, io};

use serde::Deserialize;
use serde_json::Value;

use crate::stage::skipped_entry;

pub const CASES: &str = "evals/cases";

pub struct Case {
    pub name: String,
    pub expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    // the sources the run binds, one or several
    #[serde(rename = "source")]
    pub sources: Vec<Source>,
    #[serde(default, rename = "requirement")]
    pub requirements: Vec<Item>,
    #[serde(default, rename = "criterion")]
    pub criteria: Vec<Item>,
    #[serde(default, rename = "decision")]
    pub decisions: Vec<Item>,
    // the spans no accepted requirement claim should anchor within: the
    // readiness and lifecycle code whose behaviour is the design's, so a
    // run that still states it is read apart from one that merely finds less
    #[serde(default, rename = "absent")]
    pub absent: Vec<Item>,
    // the requirements the engine reconciles across the sources, exactly
    #[serde(default, rename = "behaviour")]
    pub behaviours: Vec<Behaviour>,
    // a hand-written omnia guest of the same behaviour, relative to the
    // repository root, which a build through `omnia` is compared with and
    // whose recorded fixtures the built guest is driven through
    pub reference: Option<String>,
}

// One source of the run: the adapter it runs under and what it is bound to,
// with the stems its accepted requirement claims lead with, exactly, and
// the (entry, stem) pairs its survey decides, exactly.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    // the name the specification cites the source by; the adapter's when
    // the line is absent, as `emery` names it
    name: Option<String>,
    // the adapter the source runs under, by crate name
    pub adapter: String,
    // the fixture tree, relative to the repository root
    pub fixture: Option<String>,
    // an inline value in the fixture's place
    pub description: Option<String>,
    // the `[[source]] rank` the staged `emery.toml` carries; absent, the
    // adapter's kind decides
    pub rank: Option<NonZeroU32>,
    #[serde(default)]
    pub stems: Vec<String>,
    #[serde(default, rename = "surface")]
    pub surfaces: Vec<ExpectedSurface>,
}

impl Source {
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.adapter)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedSurface {
    pub entry: String,
    pub stem: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    // the source the item is read from; the case's one source when absent
    #[serde(default)]
    pub source: Option<String>,
    // the stem — or any one of the stems — a meeting claim's id leads with
    // for the match to count as `in stem`; none grades the anchor alone
    #[serde(default)]
    pub stem: Option<Stems>,
    pub anchor: String,
    pub gloss: String,
}

// One behaviour the specification states once, however many sources state
// it: the claims that contribute, each at its source and anchor, the status
// the engine's authority rule gives the requirement, and, where the status
// is a disagreement, the source whose statement loses — so a `divergence`
// between two other sources does not pass for the one planted.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Behaviour {
    pub gloss: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    loser: Option<String>,
    pub claims: Vec<ExpectedClaim>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedClaim {
    pub source: String,
    pub anchor: String,
}

impl Behaviour {
    pub fn label(&self) -> String {
        let claims: Vec<String> = self
            .claims
            .iter()
            .map(|claim| format!("{}:`{}`", claim.source, claim.anchor))
            .collect();
        format!("{} — {}", claims.join(", "), self.gloss)
    }

    // The status expected, with the loser where one is named: `divergence`,
    // `divergence losing faq`.
    pub fn expected_status(&self) -> Option<String> {
        let status = self.status.as_ref()?;
        Some(
            self.loser
                .as_ref()
                .map_or_else(|| status.clone(), |loser| format!("{status} losing {loser}")),
        )
    }

    // Whether a requirement holding the behaviour carries the status
    // expected and, where a loser is named, a loser note from that source.
    pub fn decided(&self, requirement: &Value) -> bool {
        let Some(status) = &self.status else { return true };
        requirement["status"] == status.as_str()
            && self.loser.as_deref().is_none_or(|loser| losers_of(requirement).any(|s| s == loser))
    }

    // What a requirement holding the behaviour decided, read the way
    // `expected_status` spells it, so a mismatch reads against it.
    pub fn outcome(&self, requirement: &Value) -> Option<String> {
        let status = requirement["status"].as_str()?;
        if self.loser.is_none() {
            return Some(status.to_owned());
        }
        let losers: BTreeSet<&str> = losers_of(requirement).collect();
        if losers.is_empty() {
            return Some(status.to_owned());
        }
        let losers: Vec<&str> = losers.into_iter().collect();
        Some(format!("{status} losing {}", losers.join(", ")))
    }
}

// The sources whose statements a requirement lists as losers, as `emery show
// --format json` carries them.
fn losers_of(requirement: &Value) -> impl Iterator<Item = &str> {
    requirement["losers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|loser| loser["sources"].as_array())
        .flatten()
        .filter_map(Value::as_str)
}

// One stem, or a list where the code is shared — a guard two routers
// declare, one declared in the bootstrap's module and applied per route —
// and a requirement at it is rightly placed under either.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Stems {
    One(String),
    Any(Vec<String>),
}

impl Stems {
    pub fn holds(&self, stem: &str) -> bool {
        match self {
            Self::One(one) => one == stem,
            Self::Any(any) => any.iter().any(|one| one == stem),
        }
    }
}

impl Display for Stems {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::One(one) => f.write_str(one),
            Self::Any(any) => f.write_str(&any.join("|")),
        }
    }
}

impl Expected {
    pub fn adapters(&self) -> impl Iterator<Item = &str> {
        self.sources.iter().map(|source| source.adapter.as_str())
    }

    pub fn trees(&self) -> impl Iterator<Item = &str> {
        self.sources
            .iter()
            .filter_map(|source| source.fixture.as_deref())
            .chain(self.reference.as_deref())
    }

    // The source an item names, or the case's one source when it names none.
    pub fn source(&self, named: Option<&str>) -> Result<&Source, String> {
        match named {
            Some(name) => self
                .sources
                .iter()
                .find(|source| source.name() == name)
                .ok_or_else(|| format!("names no source of the case: `{name}`")),
            None if self.sources.len() == 1 => Ok(&self.sources[0]),
            None => Err("names no source, and the case has several".to_owned()),
        }
    }

    // What the shape alone does not hold: one content key per source, each
    // source named once, and every item and claim at a source of the case.
    pub fn check(&self) -> Result<(), String> {
        if self.sources.is_empty() {
            return Err("names no `[[source]]`".to_owned());
        }
        if let Some(reference) = &self.reference
            && (reference.is_empty() || Path::new(reference).is_absolute())
        {
            return Err(format!(
                "names `reference = \"{reference}\"`, which is not a path relative to the \
                 repository root"
            ));
        }
        let mut names = BTreeSet::new();
        for source in &self.sources {
            if source.fixture.is_some() == source.description.is_some() {
                return Err(format!(
                    "source `{}` has one of `fixture` and `description`, not both or neither",
                    source.name()
                ));
            }
            if !names.insert(source.name()) {
                return Err(format!("source `{}` is named twice", source.name()));
            }
        }
        for (kind, items) in [
            ("requirement", &self.requirements),
            ("criterion", &self.criteria),
            ("decision", &self.decisions),
            ("absent", &self.absent),
        ] {
            for item in items {
                self.source(item.source.as_deref())
                    .map_err(|why| format!("{kind} `{}` {why}", item.anchor))?;
            }
        }
        for item in &self.absent {
            if item.stem.is_some() {
                return Err(format!(
                    "absent `{}` names a stem; a span no requirement anchors in has none",
                    item.anchor
                ));
            }
        }
        for behaviour in &self.behaviours {
            if behaviour.claims.is_empty() {
                return Err(format!("behaviour `{}` names no claim", behaviour.gloss));
            }
            for claim in &behaviour.claims {
                self.source(Some(&claim.source))
                    .map_err(|why| format!("behaviour `{}` {why}", behaviour.gloss))?;
            }
            if let Some(status) = &behaviour.status
                && !matches!(status.as_str(), "agreed" | "unknown" | "divergence" | "conflict")
            {
                return Err(format!(
                    "behaviour `{}` expects status `{status}`; the statuses are `agreed`, \
                     `unknown`, `divergence`, and `conflict`",
                    behaviour.gloss
                ));
            }
            if let Some(loser) = &behaviour.loser {
                if !matches!(behaviour.status.as_deref(), Some("divergence" | "conflict")) {
                    return Err(format!(
                        "behaviour `{}` names a loser under no disagreement; `loser` goes with \
                         status `divergence` or `conflict`",
                        behaviour.gloss
                    ));
                }
                if !behaviour.claims.iter().any(|claim| &claim.source == loser) {
                    return Err(format!(
                        "behaviour `{}` names loser `{loser}`, which none of its claims is from",
                        behaviour.gloss
                    ));
                }
            }
        }
        Ok(())
    }
}

// Every case under `evals/cases/`, or the ones the filter names — a name is
// a case's or an adapter's, which selects every case with a source running
// under it.
pub fn cases(
    root: &Path, filter: &BTreeSet<String>,
) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for entry in fs::read_dir(root.join(CASES))? {
        let dir = entry?.path();
        let Some(name) = dir.file_name().and_then(|name| name.to_str()) else { continue };
        if !dir.is_dir() {
            continue;
        }
        let expected = fs::read_to_string(dir.join("expected.toml"))
            .map_err(|error| format!("{name}: expected.toml: {error}"))?;
        let expected: Expected =
            toml::from_str(&expected).map_err(|error| format!("{name}: {error}"))?;
        expected.check().map_err(|error| format!("{name}: expected.toml {error}"))?;
        let selected = filter.is_empty()
            || filter.contains(name)
            || expected.adapters().any(|adapter| filter.contains(adapter));
        if !selected {
            continue;
        }
        // a fixture the checkout lacks (the vendored ones are gitignored until
        // fetched) skips its case unless the case was asked for by name
        let absent = expected
            .sources
            .iter()
            .filter_map(|source| source.fixture.as_deref())
            .find(|fixture| !root.join(fixture).is_dir());
        if let Some(fixture) = absent {
            if filter.contains(name) {
                return Err(format!("{name}: no fixture at `{fixture}`").into());
            }
            eprintln!("eval: `{name}` skipped, no fixture at `{fixture}`");
            continue;
        }
        for tree in expected.trees() {
            let path = root.join(tree);
            if path.is_dir() {
                check_tree(&path, tree).map_err(|error| format!("{name}: {error}"))?;
            }
        }
        cases.push(Case {
            name: name.to_owned(),
            expected,
        });
    }
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    if cases.is_empty() {
        return Ok(cases);
    }
    let unmatched: Vec<&String> = filter
        .iter()
        .filter(|name| {
            !cases.iter().any(|case| {
                case.name == **name || case.expected.adapters().any(|adapter| adapter == *name)
            })
        })
        .collect();
    if !unmatched.is_empty() {
        return Err(format!("no case or adapter under `{CASES}` named {unmatched:?}").into());
    }

    Ok(cases)
}

// Holds every staged file of a fixture or reference tree at `dir` to the
// credential gate before any case is accepted or run.
pub fn check_tree(dir: &Path, label: &str) -> Result<(), String> {
    let mut findings = Vec::new();
    check_tree_files(dir, dir, label, &mut findings)
        .map_err(|error| format!("{label}: {error}"))?;
    if findings.is_empty() {
        Ok(())
    } else {
        findings.sort();
        Err(findings.join("; "))
    }
}

fn check_tree_files(
    root: &Path, dir: &Path, label: &str, findings: &mut Vec<String>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if skipped_entry(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            check_tree_files(root, &path, label, findings)?;
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else { continue };
        let parts: Vec<String> = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        let rel = parts.join("/");
        let what = match named_secret(&rel) {
            Some(what) => Some(what.to_owned()),
            None => carried_secret(&rel, &fs::read(&path)?),
        };
        if let Some(what) = what {
            findings.push(format!("{label}/{rel}: {what}"));
        }
    }
    Ok(())
}

// Files that carry a credential by name, matched on the final path segment;
// `.npmrc` is read for `_authToken=` instead since a fixture may set
// `legacy-peer-deps` there, and `.env.example` / `.sample` / `.template`
// document variable names alone.
const SECRET_CARRIERS: [&str; 9] = [
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    ".netrc",
    ".git-credentials",
    ".pgpass",
    ".pypirc",
    ".env",
];

pub fn named_secret(path: &str) -> Option<&'static str> {
    let mut segments = path.rsplit('/');
    let name = segments.next()?;
    let env_file = name.starts_with(".env.")
        && !matches!(name, ".env.example" | ".env.sample" | ".env.template");
    if SECRET_CARRIERS.contains(&name)
        || env_file
        || (name == "credentials" && segments.next() == Some(".aws"))
    {
        return Some("a file that carries a credential by name");
    }
    None
}

#[derive(Clone, Copy)]
enum SecretRun {
    Upper,
    Alnum,
    Token,
    Base64,
}

impl SecretRun {
    const fn admits(self, byte: u8) -> bool {
        match self {
            Self::Upper => byte.is_ascii_uppercase() || byte.is_ascii_digit(),
            Self::Alnum => byte.is_ascii_alphanumeric(),
            Self::Token => byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-',
            Self::Base64 => {
                byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/' || byte == b'='
            }
        }
    }
}

struct SecretShape {
    prefix: &'static [u8],
    run: SecretRun,
    least: usize,
    what: &'static str,
}

const SECRET_SHAPES: [SecretShape; 23] = [
    SecretShape {
        prefix: b"AKIA",
        run: SecretRun::Upper,
        least: 16,
        what: "an AWS access key id",
    },
    SecretShape {
        prefix: b"ASIA",
        run: SecretRun::Upper,
        least: 16,
        what: "an AWS access key id",
    },
    SecretShape {
        prefix: b"ghp_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"gho_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"ghu_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"ghs_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"ghr_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"github_pat_",
        run: SecretRun::Token,
        least: 22,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"npm_",
        run: SecretRun::Alnum,
        least: 36,
        what: "an npm token",
    },
    SecretShape {
        prefix: b"_authToken=",
        run: SecretRun::Token,
        least: 20,
        what: "an npm `_authToken` line",
    },
    SecretShape {
        prefix: b"xoxa-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxb-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxp-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxr-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxs-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"sk_live_",
        run: SecretRun::Alnum,
        least: 24,
        what: "a Stripe live key",
    },
    SecretShape {
        prefix: b"rk_live_",
        run: SecretRun::Alnum,
        least: 24,
        what: "a Stripe live key",
    },
    SecretShape {
        prefix: b"AIza",
        run: SecretRun::Token,
        least: 35,
        what: "a Google API key",
    },
    SecretShape {
        prefix: b"glpat-",
        run: SecretRun::Token,
        least: 20,
        what: "a GitLab token",
    },
    SecretShape {
        prefix: b"NRAK-",
        run: SecretRun::Upper,
        least: 27,
        what: "a New Relic key",
    },
    SecretShape {
        prefix: b"NRII-",
        run: SecretRun::Token,
        least: 27,
        what: "a New Relic key",
    },
    SecretShape {
        prefix: b"AccountKey=",
        run: SecretRun::Base64,
        least: 20,
        what: "an Azure Storage account key",
    },
    SecretShape {
        prefix: b"SharedAccessKey=",
        run: SecretRun::Base64,
        least: 20,
        what: "an Azure shared access key",
    },
];

pub fn carried_secret(path: &str, bytes: &[u8]) -> Option<String> {
    if pem_private_key(bytes) {
        return Some("a PEM private key block".to_owned());
    }
    if let Some(shape) = SECRET_SHAPES.iter().find(|shape| shape.matches(bytes)) {
        return Some(shape.what.to_owned());
    }
    if azure_storage_key(bytes) {
        return Some("an Azure Storage account key".to_owned());
    }
    parameter_secret(path, bytes)
}

fn pem_private_key(bytes: &[u8]) -> bool {
    const HEAD: &[u8] = b"-----BEGIN ";
    const DASHES: &[u8] = b"-----";
    const PRIVATE: &[u8] = b"PRIVATE KEY";

    byte_occurrences(bytes, HEAD).any(|start| {
        let rest = &bytes[start + HEAD.len()..];
        byte_occurrences(rest, DASHES).next().is_some_and(|end| {
            let label = &rest[..end];
            label.ends_with(PRIVATE) && !label.contains(&b'\n')
        })
    })
}

impl SecretShape {
    fn matches(&self, bytes: &[u8]) -> bool {
        byte_occurrences(bytes, self.prefix).any(|start| {
            let rest = &bytes[start + self.prefix.len()..];
            let run = rest.iter().take_while(|byte| self.run.admits(**byte)).count();
            run >= self.least && rest.get(run) != Some(&b'/')
        })
    }
}

// A 512-bit Azure Storage account key literal: 86 base64 characters and
// `==` bounded by a quote, `=`, or whitespace, never the `sha512-` prefix
// of a lockfile's integrity hash.
fn azure_storage_key(bytes: &[u8]) -> bool {
    byte_occurrences(bytes, b"==").any(|end| {
        if end < 86 {
            return false;
        }
        let start = end - 86;
        let bounded_before = start == 0
            || matches!(bytes[start - 1], b'"' | b'\'' | b'=' | b' ' | b'\t' | b'\r' | b'\n');
        let bounded_after = end + 2 == bytes.len()
            || matches!(bytes[end + 2], b'"' | b'\'' | b',' | b' ' | b'\t' | b'\r' | b'\n');
        let body = &bytes[start..end];
        bounded_before
            && bounded_after
            && body
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/')
            && body.iter().any(|byte| *byte == b'+' || *byte == b'/')
    })
}

// A cloud parameter file (`parameters.json` or `*.parameters.json`) whose
// key, secret, token, password, or connection-string entry holds a
// non-empty literal rather than an empty placeholder or expression.
fn parameter_secret(path: &str, bytes: &[u8]) -> Option<String> {
    let name = path.rsplit('/').next()?;
    if name != "parameters.json" && !name.ends_with(".parameters.json") {
        return None;
    }
    let root: Value = serde_json::from_slice(bytes).ok()?;
    let object = root.get("parameters").and_then(Value::as_object).or_else(|| root.as_object())?;
    for (key, entry) in object {
        if secret_parameter_key(key) && has_credential_literal(entry) {
            return Some(format!("parameter `{key}` carries a credential literal"));
        }
    }
    None
}

fn secret_parameter_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    if lower.contains("secret_name") || lower.contains("key_name") {
        return false;
    }
    lower.ends_with("connection_string")
        || lower.ends_with("connectionstring")
        || lower
            .rsplit(['_', '-'])
            .next()
            .is_some_and(|last| matches!(last, "key" | "secret" | "token" | "password" | "passwd"))
}

fn has_credential_literal(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with('[')
                && !trimmed.starts_with("${")
                && !trimmed.starts_with("@Microsoft.KeyVault(")
        }
        Value::Object(map) => map.values().any(has_credential_literal),
        _ => false,
    }
}

fn byte_occurrences<'a>(bytes: &'a [u8], needle: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    (0..bytes.len()).filter(move |start| bytes[*start..].starts_with(needle))
}

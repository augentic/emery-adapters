//! Grades a run's claims, surfaces, absent spans, and behaviours against the case's expectations.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};

use emery_sdk::{Anchor, Claim, ClaimKind};
use serde_json::Value;

use crate::case::{Behaviour, Case, Expected, ExpectedClaim, ExpectedSurface, Item};
use crate::run::Run;

pub struct Report<'a> {
    pub case: &'a Case,
    // one entry per run: its attempts up the ladder, the first rung's first
    pub runs: Vec<Vec<Attempt>>,
}

pub struct Attempt {
    pub run: Run,
    pub grade: Grade,
}

pub struct Grade {
    pub requirements: Recall,
    pub criteria: Recall,
    pub decisions: Recall,
    pub absent: Absent,
    pub behaviours: Behaviours,
    // what each source's survey and stems read against its expectations
    pub sources: Vec<SourceGrade>,
    // where the specification's own words name a technology; expected empty
    pub leaks: Vec<String>,
}

// The accepted requirement claims anchored within the spans expected to
// state none: one hit per claim, naming the span and the claim.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Absent {
    pub expected: usize,
    pub hits: Vec<String>,
}

pub struct SourceGrade {
    pub name: String,
    pub surfaces: Surfaces,
    pub missing_stems: Vec<String>,
    pub extra_stems: Vec<String>,
}

#[derive(Default)]
pub struct Recall {
    pub matched: usize,
    // of the matched items naming a stem, those met by a claim under it
    placed: usize,
    pub expected: usize,
    pub missed: Vec<String>,
    pub misplaced: Vec<String>,
}

// The (entry, stem) pairs the survey decided against the ones expected: the
// pairs met, the entries found under another stem, the pairs met by nothing
// at their entry, and the surveyed surfaces no expected entry accounts for.
#[derive(Default)]
pub struct Surfaces {
    // whether the source's adapter logged a survey at all: one that cuts its
    // seams without a turn decides no surface, and reads as `no survey`
    // rather than as a survey that missed every one
    pub surveyed: bool,
    matched: usize,
    pub expected: usize,
    pub restemmed: Vec<String>,
    pub missed: Vec<String>,
    pub extra: Vec<String>,
}

// The specification's requirements against the behaviours expected: each
// behaviour met by a requirement citing one of its claims — or holding one
// as a criterion under a claim it cites — split across two or more citing
// requirements, or met by none; a requirement citing two behaviours is a
// wrong merge; a met behaviour whose claims are not all held is partial; and
// the status the engine gave is read against the one expected. A criterion
// counts toward neither split nor wrong merge: it is in the specification
// under its requirement, hidden from no one, and no grouping can join it to
// a requirement of the other source.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Behaviours {
    pub expected: usize,
    pub met: usize,
    pub split: Vec<String>,
    pub wrong: Vec<String>,
    pub missed: Vec<String>,
    pub partial: Vec<String>,
    // the behaviours no requirement cites directly, met through a criterion
    // alone: the kind the model chose, which the count would otherwise hide
    pub by_criterion: Vec<String>,
    // the behaviours one requirement cites while another holds them as a
    // criterion: stated across kinds, which the engine cannot reconcile
    pub across: Vec<String>,
    pub status_expected: usize,
    pub status_met: usize,
    pub status_wrong: Vec<String>,
    // the requirements the engine pre-merged on a byte-equal id across
    // sources, by their subject and the id
    pub premerged: Vec<String>,
}

pub fn grade(expected: &Expected, run: &Run) -> Grade {
    let sources = expected
        .sources
        .iter()
        .map(|source| {
            let observed = run.stems_of(source.name());
            let wanted: BTreeSet<&str> = source.stems.iter().map(String::as_str).collect();
            SourceGrade {
                name: source.name().to_owned(),
                surfaces: surfaces(
                    &source.surfaces,
                    &run.pairs_of(source.name()),
                    run.surveyed.contains_key(source.name()),
                ),
                missing_stems: wanted
                    .difference(&observed)
                    .map(|stem| (*stem).to_owned())
                    .collect(),
                extra_stems: observed.difference(&wanted).map(|stem| (*stem).to_owned()).collect(),
            }
        })
        .collect();

    Grade {
        requirements: recall(&expected.requirements, expected, run, ClaimKind::Requirement),
        criteria: recall(&expected.criteria, expected, run, ClaimKind::Criterion),
        decisions: recall(&expected.decisions, expected, run, ClaimKind::Decision),
        absent: absent(&expected.absent, expected, run),
        behaviours: behaviours(&expected.behaviours, run),
        sources,
        leaks: run.leaks(),
    }
}

// An absent span is hit by every accepted requirement claim of its source
// whose anchor is held to it (`held`): a claim half or more within the
// span states what the span does, while one a line wider than it does not.
// A span naming no lines is hit by any claim in its file. Each hit is
// labelled by the span and the claim, so the card's reading names the
// passage of the prompt the claim passed through.
pub fn absent(items: &[Item], expected: &Expected, run: &Run) -> Absent {
    let mut absent = Absent {
        expected: items.len(),
        ..Absent::default()
    };
    for item in items {
        let source = match (&item.source, expected.sources.len()) {
            (Some(source), _) => format!("{source}:"),
            (None, 1) => String::new(),
            (None, _) => "?:".to_owned(),
        };
        let (Ok(of), Ok(span)) =
            (expected.source(item.source.as_deref()), Anchor::parse(&item.anchor))
        else {
            continue;
        };
        for claim in run.claims_of(of.name(), ClaimKind::Requirement) {
            let Some(Ok(anchor)) = claim.anchor() else { continue };
            if held(anchor, span) {
                absent.hits.push(format!(
                    "{source}`{}` — {} (`{}` at `{}`)",
                    item.anchor,
                    item.gloss,
                    claim.id.as_deref().unwrap_or("?"),
                    claim.path.as_deref().unwrap_or("?"),
                ));
            }
        }
    }

    absent
}

// An expected item is met by a claim of its kind, from its source, whose
// anchor overlaps its own — same file, meeting line ranges, a whole-file
// anchor meeting any. Paraphrase never enters into it; the stem is counted
// apart, so a survey that shapes the estate differently still shows what it
// mined.
fn recall(items: &[Item], expected: &Expected, run: &Run, kind: ClaimKind) -> Recall {
    let mut recall = Recall {
        expected: items.len(),
        ..Recall::default()
    };
    for item in items {
        let label = || {
            let stem = item.stem.as_ref().map_or(String::new(), |stems| format!("{stems} · "));
            let source = match (&item.source, expected.sources.len()) {
                (Some(source), _) => format!("{source}:"),
                (None, 1) => String::new(),
                (None, _) => "?:".to_owned(),
            };
            format!("{stem}{source}`{}` — {}", item.anchor, item.gloss)
        };
        let Ok(source) = expected.source(item.source.as_deref()) else {
            recall.missed.push(format!("{} (no such source)", label()));
            continue;
        };
        let Ok(wanted) = Anchor::parse(&item.anchor) else {
            recall.missed.push(format!("{} (unparseable anchor)", label()));
            continue;
        };
        let meeting: Vec<&Claim> = run
            .claims_of(source.name(), kind)
            .filter(|claim| {
                claim.anchor().and_then(Result::ok).is_some_and(|got| overlaps(got, wanted))
            })
            .collect();
        if meeting.is_empty() {
            recall.missed.push(label());
            continue;
        }
        recall.matched += 1;
        if let Some(stems) = &item.stem {
            let placed = meeting.iter().any(|claim| {
                claim.id.as_deref().and_then(self::stem).is_some_and(|s| stems.holds(s))
            });
            if placed {
                recall.placed += 1;
            } else {
                let under: BTreeSet<&str> = meeting
                    .iter()
                    .filter_map(|claim| claim.id.as_deref().and_then(self::stem))
                    .collect();
                recall.misplaced.push(format!("{} (under {:?})", label(), under));
            }
        }
    }

    recall
}

// An expected surface is met when the survey decided a surface at its entry
// under its stem; one the survey decided at the entry under another stem is
// restemmed — found, and stemmed differently, which is how a survey that is
// present but wrong reads apart from one that is absent. The names are the
// survey's own and never graded.
fn surfaces(
    expected: &[ExpectedSurface], observed: &BTreeSet<(&str, &str)>, surveyed: bool,
) -> Surfaces {
    let wanted: BTreeSet<(&str, &str)> =
        expected.iter().map(|surface| (surface.entry.as_str(), surface.stem.as_str())).collect();
    let label = |(entry, stem): &(&str, &str)| format!("`{entry}` · {stem}");

    let mut open: Vec<(&str, &str)> = observed.difference(&wanted).copied().collect();
    let mut restemmed = Vec::new();
    let mut missed = Vec::new();
    for pair in wanted.difference(observed) {
        match open.iter().position(|(entry, _)| *entry == pair.0) {
            Some(at) => {
                let (_, got) = open.remove(at);
                restemmed.push(format!("{} (as {got})", label(pair)));
            }
            None => missed.push(label(pair)),
        }
    }

    Surfaces {
        surveyed,
        matched: wanted.intersection(observed).count(),
        expected: wanted.len(),
        restemmed,
        missed,
        extra: open.iter().map(label).collect(),
    }
}

// The requirements holding one behaviour, by how: `citing` cite one of its
// claims, `covering` hold one as a criterion and cite none.
pub struct Holders {
    pub citing: Vec<usize>,
    pub covering: Vec<usize>,
}

impl Holders {
    fn all(&self) -> impl Iterator<Item = usize> + '_ {
        self.citing.iter().chain(&self.covering).copied()
    }
}

// A requirement cites a behaviour when one of its citations — a source and
// the `path` the claim anchors at, as `emery show --format json` carries
// them — is from the source of one of the behaviour's claims and is held to
// its anchor's lines (`held`); it covers one through a criterion
// (`Run::covers`). The split and wrong-merge counts read the citations alone;
// a behaviour met by covering alone, or cited by one requirement and covered
// by another, is noted, so the kind the model chose still shows.
pub fn behaviours(expected: &[Behaviour], run: &Run) -> Behaviours {
    let mut graded = Behaviours {
        expected: expected.len(),
        ..Behaviours::default()
    };
    let requirements = run.requirements();

    // which behaviours each requirement cites
    let mut citing_each: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); requirements.len()];
    for (b, behaviour) in expected.iter().enumerate() {
        let holders = run.holders(behaviour);
        for &index in &holders.citing {
            citing_each[index].insert(b);
        }
        let label = behaviour.label();
        if holders.citing.is_empty() && holders.covering.is_empty() {
            graded.missed.push(label);
            continue;
        }
        graded.met += 1;
        let subjects: Vec<String> =
            holders.citing.iter().map(|&index| run.subject(index)).collect();
        if subjects.len() > 1 {
            graded.split.push(format!("{label} (requirements {subjects:?})"));
        }
        let criteria = run.criteria_under(&holders.covering, behaviour).join(", ");
        if holders.citing.is_empty() {
            graded.by_criterion.push(format!("{label} ({criteria})"));
        } else if !holders.covering.is_empty() {
            graded.across.push(format!("{label} ({criteria}; cited by {subjects:?})"));
        }
        let unheld: Vec<String> = behaviour
            .claims
            .iter()
            .filter(|claim| !holders.all().any(|index| run.meets(&requirements[index], claim)))
            .map(|claim| format!("{}:`{}`", claim.source, claim.anchor))
            .collect();
        if !unheld.is_empty() {
            graded.partial.push(format!("{label} (uncited {})", unheld.join(", ")));
        }
        if let Some(status) = behaviour.expected_status() {
            graded.status_expected += 1;
            if holders.all().any(|index| behaviour.decided(&requirements[index])) {
                graded.status_met += 1;
            } else {
                let found: BTreeSet<String> = holders
                    .all()
                    .filter_map(|index| behaviour.outcome(&requirements[index]))
                    .collect();
                graded.status_wrong.push(format!("{label} (expected `{status}`, got {found:?})"));
            }
        }
    }
    for (index, cited) in citing_each.iter().enumerate() {
        if cited.len() > 1 {
            let glosses: Vec<&str> = cited.iter().map(|&b| expected[b].gloss.as_str()).collect();
            graded.wrong.push(format!("`{}` cites {glosses:?}", run.subject(index)));
        }
    }

    // the requirements the engine pre-merged on a byte-equal id across sources
    for (index, requirement) in requirements.iter().enumerate() {
        let mut by_id: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for cited in requirement["sources"].as_array().into_iter().flatten() {
            if let (Some(source), Some(claim)) = (cited["source"].as_str(), cited["claim"].as_str())
            {
                by_id.entry(claim).or_default().insert(source);
            }
        }
        for (id, sources) in by_id {
            if sources.len() > 1 {
                graded
                    .premerged
                    .push(format!("`{}` on `{id}` from {sources:?}", run.subject(index)));
            }
        }
    }

    graded
}

pub fn stem(id: &str) -> Option<&str> {
    id.split('.').next().filter(|stem| !stem.is_empty())
}

// `id` is `parent` or a dotted child of it, as the engine reads a criterion
// covering a requirement.
pub fn under(id: &str, parent: &str) -> bool {
    id.strip_prefix(parent).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

// Whether a requirement cites `claim`: a citation from the claim's source
// held to its lines.
pub fn cites(requirement: &Value, claim: &ExpectedClaim) -> bool {
    let Ok(wanted) = Anchor::parse(&claim.anchor) else { return false };
    citations(requirement).any(|(source, anchor)| source == claim.source && held(anchor, wanted))
}

// A citation is held to a behaviour's lines when it shares at least as many
// lines with them as it spends outside them: a whole-method anchor is not a
// statement of each branch within it, while one a line wider than the
// behaviour's is. An anchor naming no lines is held to any in its file.
pub fn held(citation: Anchor<'_>, lines: Anchor<'_>) -> bool {
    citation.path == lines.path
        && match (citation.lines, lines.lines) {
            (Some((from, to)), Some((start, end))) => {
                let shared = to.min(end).saturating_add(1).saturating_sub(from.max(start));
                shared.saturating_mul(2) >= to.saturating_sub(from).saturating_add(1)
            }
            _ => true,
        }
}

// Two anchors meet in one file when either names no lines or their ranges
// overlap.
fn overlaps(a: Anchor<'_>, b: Anchor<'_>) -> bool {
    a.path == b.path
        && match (a.lines, b.lines) {
            (Some((a, b)), Some((c, d))) => a <= d && c <= b,
            _ => true,
        }
}

// A requirement stated through a platform's mechanism: the code
// identifiers its body names that the design owns no type for, with the
// sources it cites.
#[derive(Debug, PartialEq, Eq)]
pub struct Stated {
    pub id: String,
    pub sources: Vec<String>,
    pub identifiers: Vec<String>,
}

// The requirements naming a code identifier the design owns no type for,
// in specification order. A body names one in backticks or bare, since one
// adapter's model sets code apart and another's does not. The figure is a
// proxy for a requirement stated through a platform's mechanism rather than
// its behaviour, and the list beside the count is its reading: a product
// name (`TypeORM`) has the shape, and a platform-neutral requirement names
// neither.
pub fn mechanism(requirements: &[Value], types: &BTreeSet<&str>) -> Vec<Stated> {
    requirements
        .iter()
        .filter_map(|requirement| {
            let body: Vec<&str> = requirement["body"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let mut identifiers: Vec<String> = Vec::new();
            for span in body.iter().flat_map(|line| spans(line)) {
                if identifier(span).is_some_and(|name| !types.contains(name))
                    && !identifiers.iter().any(|seen| seen == span)
                {
                    identifiers.push(span.to_owned());
                }
            }
            if identifiers.is_empty() {
                return None;
            }
            let mut sources: Vec<String> = Vec::new();
            for source in citations(requirement).map(|(source, _)| source) {
                if !sources.iter().any(|seen| seen == source) {
                    sources.push(source.to_owned());
                }
            }
            Some(Stated {
                id: requirement["id"].as_str().unwrap_or_default().to_owned(),
                sources,
                identifiers,
            })
        })
        .collect()
}

// The spans of a line an identifier may stand in: each span set in backticks,
// whole, and each bare word between them, its surrounding punctuation
// stripped and a call's parentheses kept.
fn spans(line: &str) -> impl Iterator<Item = &str> {
    line.split('`').enumerate().flat_map(|(index, part)| {
        let quoted = index % 2 == 1;
        let words: Box<dyn Iterator<Item = &str>> = if quoted {
            Box::new(std::iter::once(part))
        } else {
            Box::new(part.split_whitespace().map(bare))
        };
        words.filter(|span| !span.is_empty())
    })
}

fn bare(word: &str) -> &str {
    let mut word = word.trim_start_matches(['(', '[', '"', '\'', '*']);
    loop {
        let trimmed = word.trim_end_matches(['.', ',', ';', ':', '!', '?', ']', '"', '\'', '*']);
        let unbalanced =
            trimmed.ends_with(')') && trimmed.matches(')').count() > trimmed.matches('(').count();
        let trimmed = if unbalanced { &trimmed[..trimmed.len() - 1] } else { trimmed };
        if trimmed.len() == word.len() {
            return word;
        }
        word = trimmed;
    }
}

// The name a span spells a platform's code identifier by, when it does: a
// decorator (`@Exclude({ toPlainOnly: true })`), a call (`AuthGuard('jwt')`,
// `validate()`, `I18nContext.current()`; an English plural, `token(s)`, is
// none), or a type's name in CamelCase (`FileInterceptor`, `TypeORM`). The
// name is the span's first segment, the one a design type is known by. A
// field, an error key, a configuration key, and a function of the program's
// own (`refreshToken`, `errors.email`, `auth.secret`, `setTier`) are its
// vocabulary on any platform and none of these, nor is a single capitalised
// word, an acronym, a `SCREAMING_SNAKE` setting, a path, a route, or a
// value.
pub fn identifier(span: &str) -> Option<&str> {
    let (decorated, span) = span.strip_prefix('@').map_or((false, span), |rest| (true, rest));
    let (arguments, head) = match (span.find('('), span.ends_with(')')) {
        (Some(open), true) => (Some(&span[open + 1..span.len() - 1]), &span[..open]),
        (None, false) => (None, span),
        _ => return None,
    };
    let segment = |text: &str| {
        let mut chars = text.chars();
        chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    let segments: Vec<&str> = head.split('.').collect();
    if !segments.iter().all(|part| segment(part)) {
        return None;
    }
    let name = segments.first().copied()?;
    let typed = segments.len() == 1
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && camel_case(name);
    let called = arguments.is_some_and(|arguments| {
        arguments.is_empty()
            || arguments.starts_with(['\'', '"', '{', '[', '`'])
            || arguments.starts_with(|c: char| c.is_ascii_digit())
            || segments.len() > 1
            || camel_case(name)
    });
    (decorated || called || typed).then_some(name)
}

// A name in CamelCase: a lowercase letter directly followed by an uppercase
// one, which `Authorization`, `JWTs`, and `S3` lack.
fn camel_case(name: &str) -> bool {
    name.as_bytes()
        .windows(2)
        .any(|pair| pair[0].is_ascii_lowercase() && pair[1].is_ascii_uppercase())
}

// A requirement's citations that anchor somewhere: the source and the parsed
// `path`, as `emery show --format json` spells them.
pub fn citations(requirement: &Value) -> impl Iterator<Item = (&str, Anchor<'_>)> {
    requirement["sources"].as_array().into_iter().flatten().filter_map(|cited| {
        let source = cited["source"].as_str()?;
        let anchor = Anchor::parse(cited["path"].as_str()?).ok()?;
        Some((source, anchor))
    })
}

// Two citations anchor at one place when they come from the same source and
// name the same file, with line ranges that meet, or neither naming lines —
// the engine's rule, stricter than `overlaps`, which lets a whole-file anchor
// meet any.
pub fn cited_alike((source, a): (&str, Anchor<'_>), (other, b): (&str, Anchor<'_>)) -> bool {
    source == other
        && a.path == b.path
        && match (a.lines, b.lines) {
            (Some((start, end)), Some((other_start, other_end))) => {
                start <= other_end && other_start <= end
            }
            (None, None) => true,
            _ => false,
        }
}

impl Display for Recall {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.expected == 0 {
            return f.write_str("—");
        }
        write!(f, "{}/{} ({}%)", self.matched, self.expected, self.matched * 100 / self.expected)?;
        if self.placed + self.misplaced.len() > 0 {
            write!(f, ", {} in stem", self.placed)?;
        }
        Ok(())
    }
}

impl Display for Surfaces {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.expected == 0 {
            return f.write_str("—");
        }
        if !self.surveyed {
            return f.write_str("no survey");
        }
        write!(f, "{}/{}", self.matched, self.expected)?;
        if !self.restemmed.is_empty() {
            write!(f, " ~{}", self.restemmed.len())?;
        }
        if !self.extra.is_empty() {
            write!(f, " +{}", self.extra.len())?;
        }
        Ok(())
    }
}

// `met/expected`, then `~k` behaviours split across requirements and `!k`
// requirements merging two, each where any.
impl Display for Behaviours {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.expected == 0 {
            return f.write_str("—");
        }
        write!(f, "{}/{}", self.met, self.expected)?;
        if !self.split.is_empty() {
            write!(f, " ~{}", self.split.len())?;
        }
        if !self.wrong.is_empty() {
            write!(f, " !{}", self.wrong.len())?;
        }
        Ok(())
    }
}

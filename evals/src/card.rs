//! Renders the scorecard: a row per attempt, its notes, the ladder, and the stability across runs.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};

use emery_sdk::ClaimKind;
use serde_json::Value;

use crate::build::{Build, Integrated};
use crate::grade::{Attempt, Behaviours, Report, SourceGrade, citations, cited_alike, stem};
use crate::run::Run;
use crate::{Again, Backend, Rung, Settings};

pub fn scorecard(settings: &Settings, reports: &[Report<'_>], started: &str, name: &str) -> String {
    let ladder: Vec<String> = settings.ladder.iter().map(Rung::to_string).collect();
    let adapters: Vec<String> = settings
        .wasm
        .iter()
        .map(|(adapter, path)| format!("adapter `{adapter}` `{}`", path.display()))
        .collect();
    let built = settings.target.map_or_else(String::new, |target| {
        let jobs = settings
            .jobs
            .map_or_else(|| "the engine's default".to_owned(), |jobs| format!("`--jobs {jobs}`"));
        format!(
            " · each landed plan built through `{}` into a repository the engine created under \
             {jobs} at {} (`EVAL_BUILD_RUNG`)",
            target.name(),
            settings.build_rung
        )
    });
    let mut budget = String::new();
    for (knob, value) in &settings.knobs {
        let _ = write!(budget, " · `{knob}={value}`");
    }
    let rebuilt = settings.again.as_ref().map_or_else(String::new, |again| match again {
        Again::Rebuild { card } => format!(
            " · REBUILD: each plan is `run-1`'s of `target/eval/{card}/`, built again fresh; the \
             specify columns and completions repeat that card's"
        ),
        Again::Resume { card, wave, drifted } => {
            let commits: Vec<String> = reports
                .iter()
                .filter_map(|report| {
                    let rewound =
                        report.runs.first()?.first()?.run.build.as_ref()?.rewound.as_ref()?;
                    Some(format!(
                        "`{}` from `{}` ({})",
                        report.case.name,
                        short(&rewound.commit),
                        rewound.subject
                    ))
                })
                .collect();
            let admitted = if *drifted {
                "; the seed admitted by `--drifted`, the template having moved under it"
            } else {
                ""
            };
            format!(
                " · RESUME: each plan is `run-1`'s of `target/eval/{card}/`, its label moved to \
                 the last commit of wave {} and built on from wave {wave} ({}){admitted}; the \
                 specify columns and completions repeat that card's",
                wave - 1,
                commits.join(", ")
            )
        }
    });
    let (timeout, inactivity) = settings.backend.rung();
    let mut card = format!(
        "# Eval {started}\n\nmodel `{}` (`{}`) · backend {} · emery `{}` · {} · {} runs per case \
         · ladder {} (`{timeout}`/`{inactivity}`, climbed on a `timeout` or `inactive` \
         completion; the columns are the first rung's){budget}{built}{rebuilt} · runs \
         under `target/eval/{name}/`\n",
        settings.model,
        settings.backend.model().0,
        settings.backend.name(),
        settings.emery.display(),
        adapters.join(" · "),
        settings.runs,
        ladder.join(" → "),
    );
    for report in reports {
        let _ = write!(
            card,
            "{}",
            Card {
                report,
                ladder: &settings.ladder,
                backend: settings.backend,
                built: settings.target.is_some(),
            }
        );
    }

    card
}

struct Card<'a> {
    report: &'a Report<'a>,
    ladder: &'a [Rung],
    backend: Backend,
    // whether the run built each plan, so the rows carry the build columns
    built: bool,
}

impl Display for Card<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Report { case, runs } = self.report;
        let sources: Vec<String> = case
            .expected
            .sources
            .iter()
            .map(|source| {
                let bound = match (&source.fixture, &source.description) {
                    (Some(fixture), _) => format!("fixture `{fixture}`"),
                    _ => "inline".to_owned(),
                };
                format!("source `{}` (adapter `{}`, {bound})", source.name(), source.adapter)
            })
            .collect();
        writeln!(f, "\n## {}\n\n{}\n", case.name, sources.join(" · "))?;

        // one row per run, at the first rung
        let build_columns = if self.built {
            " built | verified | build wall | speedup | covered | host checks | scenarios named |"
        } else {
            ""
        };
        writeln!(
            f,
            "| run | exit | wall | completions | input | cached | output | reasoning | claims | \
             req | crit | recall req | recall crit | recall dec | surfaces | stems | behaviours | \
             conflicts | divergences | unknown | covered | then [unknown] | slices | waves | \
             design blocks (types) | mechanism | leaks |{build_columns}"
        )?;
        writeln!(f, "|{}", " --- |".repeat(if self.built { 34 } else { 27 }))?;
        for attempts in runs {
            if let Some(first) = attempts.first() {
                row(f, first, case.expected.sources.len() > 1)?;
                if self.built {
                    build_cells(f, first)?;
                }
                writeln!(f)?;
            }
        }

        // what each attempt missed and spent
        for attempts in runs {
            for (index, attempt) in attempts.iter().enumerate() {
                notes(f, attempt, index > 0)?;
            }
        }

        // where each run landed on the ladder
        if self.ladder.len() > 1 {
            ladder(f, runs, self.ladder, self.backend)?;
        }

        // stability across runs: at the first rung when two landed there,
        // else across the attempts that landed anywhere
        let landed: Vec<&Attempt> = runs
            .iter()
            .filter_map(|attempts| attempts.first())
            .filter(|attempt| attempt.run.exit == Some(0))
            .collect();
        if landed.len() >= 2 {
            let label = format!("across {} runs at {}", landed.len(), self.ladder[0]);
            stability(f, &label, &landed)?;
        } else {
            let landed: Vec<&Attempt> = runs
                .iter()
                .filter_map(|attempts| attempts.last())
                .filter(|attempt| attempt.run.exit == Some(0))
                .collect();
            if landed.len() >= 2 {
                let label = format!("across the {} attempts that landed", landed.len());
                stability(f, &label, &landed)?;
            }
        }

        Ok(())
    }
}

// A cell over every source: each source's reading, led by its name where
// the case has several.
fn per_source(
    grades: &[SourceGrade], named: bool, cell: impl Fn(&SourceGrade) -> String,
) -> String {
    grades
        .iter()
        .map(|grade| if named { format!("{} {}", grade.name, cell(grade)) } else { cell(grade) })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn row(f: &mut Formatter<'_>, attempt: &Attempt, named: bool) -> fmt::Result {
    let Attempt { run, grade } = attempt;
    let tokens = run.tokens();
    let (blocks, types) = run.design_blocks();
    let stems = per_source(&grade.sources, named, |source| {
        if source.missing_stems.is_empty() && source.extra_stems.is_empty() {
            "exact".to_owned()
        } else {
            format!("-{} +{}", source.missing_stems.len(), source.extra_stems.len())
        }
    });
    let surfaces = per_source(&grade.sources, named, |source| source.surfaces.to_string());
    let stated = run.mechanism();
    let mechanism = per_source(&grade.sources, named, |source| {
        let named = stated.iter().filter(|stated| stated.sources.contains(&source.name)).count();
        format!("{named}/{}", run.citing(&source.name))
    });

    write!(
        f,
        "| {} | {} | {}s | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | \
         {} | {} | {} | {} | {} | {} | {} ({}) | {mechanism} | {} |",
        run.n,
        match (run.exit, run.died_in()) {
            (Some(0), _) => "0".to_owned(),
            (Some(code), Some(label)) => format!("{code} in `{label}`"),
            (Some(code), None) => code.to_string(),
            (None, _) => "killed".to_owned(),
        },
        run.wall.as_secs(),
        run.completions.len(),
        tokens.input,
        tokens.cached,
        tokens.output,
        tokens.reasoning,
        run.evidence.iter().map(|(_, evidence)| evidence.claims.len()).sum::<usize>(),
        run.claims(ClaimKind::Requirement).count(),
        run.claims(ClaimKind::Criterion).count(),
        grade.requirements,
        grade.criteria,
        grade.decisions,
        surfaces,
        stems,
        grade.behaviours,
        run.status("conflict"),
        run.status("divergence"),
        run.status("unknown"),
        run.covered(),
        run.unknown_outcomes(),
        run.slices(),
        run.waves(),
        blocks,
        types,
        grade.leaks.len(),
    )
}

// The seven build cells after a row's own: what the build merged, verified,
// took, saved over a serial walk, and covered, then what the host's checks
// and the tests say of the tree it left; `—` where no plan was built, and
// in the last two where the build left no tree.
fn build_cells(f: &mut Formatter<'_>, attempt: &Attempt) -> fmt::Result {
    let run = &attempt.run;
    let Some(build) = &run.build else {
        return write!(f, " — | — | — | — | — | — | — |");
    };
    let integrated = build.integrated();
    let resolved = integrated.iter().filter(|slice| !slice.resolved.is_empty()).count();
    let mut merged = format!("{}/{}", integrated.len(), run.slices());
    if resolved > 0 {
        let _ = write!(merged, " ~{resolved}");
    }
    if let Some(code) = build.code() {
        let _ = write!(merged, " `{code}`");
    }
    let verified = format!("{}/{}", build.verified(), build.waves());
    let wall = match build.exit {
        Some(0) => format!("{}s", build.wall.as_secs()),
        Some(code) => format!("{}s exit {code}", build.wall.as_secs()),
        None => format!("{}s killed", build.wall.as_secs()),
    };
    // a landed build reads against the plan's factor; one that did not says
    // how far it got, since a partial path compares with nothing
    let against = match (build.landed(), run.wave_count()) {
        (_, 0) => String::new(),
        (true, waves) => format!(" (plan {})", factor(run.slices() * 100 / waves)),
        (false, waves) => format!(" ({} of {waves} waves)", build.waves()),
    };
    let speedup = build
        .speedup()
        .map_or_else(|| "—".to_owned(), |speedup| format!("{}{against}", factor(speedup)));
    let (covered, all) = build.covered();
    let (host, scenarios) = build.fidelity.as_ref().map_or_else(
        || ("—".to_owned(), "—".to_owned()),
        |fidelity| (fidelity.host_cell(), fidelity.scenarios_cell()),
    );
    write!(
        f,
        " {merged} | {verified} | {wall} | {speedup} | {covered}/{all} | {host} | {scenarios} |"
    )
}

// A ratio in hundredths as the card spells it, `1.40×`.
pub fn factor(hundredths: usize) -> String {
    format!("{}.{:02}×", hundredths / 100, hundredths % 100)
}

fn notes(f: &mut Formatter<'_>, attempt: &Attempt, climbed: bool) -> fmt::Result {
    let Attempt { run, grade } = attempt;
    let at = if climbed { format!(" @ {}", run.rung) } else { String::new() };
    let graded = !run.evidence.is_empty();
    let named = grade.sources.len() > 1;
    let led = |source: &str| if named { format!("{source}: ") } else { String::new() };
    if run.exit == Some(0) {
        let stems: Vec<String> = grade
            .sources
            .iter()
            .map(|source| format!("{}{:?}", led(&source.name), run.stems_of(&source.name)))
            .collect();
        writeln!(f, "\nRun {}{at}: stems {}", run.n, stems.join(" · "))?;
    } else {
        // the turn the run died in, and whether it left claims to grade
        let died = run.died_in().map_or(String::new(), |label| format!(" in `{label}`"));
        let left = if graded {
            format!(", {} seams accepted — the claims are graded below", run.evidence.len())
        } else {
            ", no claim accepted".to_owned()
        };
        writeln!(f, "\nRun {}{at} failed{died}{left}:\n\n```\n{}\n```", run.n, run.tail)?;
    }
    if let Some(dead) = &run.dead {
        writeln!(f, "- put again once: the first attempt {dead}, before any claim")?;
    }

    for source in &grade.sources {
        survey_notes(f, run, source, &led(&source.name), graded)?;
    }
    // the claims
    if graded {
        if let Some(landing) = run.landing() {
            writeln!(f, "- requirement anchors: {landing}")?;
        }
        for (kind, recall) in [
            ("requirement", &grade.requirements),
            ("criterion", &grade.criteria),
            ("decision", &grade.decisions),
        ] {
            for missed in &recall.missed {
                writeln!(f, "- {kind} missed: {missed}")?;
            }
            for misplaced in &recall.misplaced {
                writeln!(f, "- {kind} misplaced: {misplaced}")?;
            }
        }
        if grade.absent.expected > 0 {
            writeln!(f, "- absent hits: {}", grade.absent.hits.len())?;
            for hit in &grade.absent.hits {
                writeln!(f, "- absent hit: {hit}")?;
            }
        }
    }
    if run.spec.is_some() {
        mechanism_notes(f, run, &grade.sources, &led)?;
    }
    for leak in &grade.leaks {
        writeln!(f, "- leak: {leak}")?;
    }
    if grade.behaviours.expected > 0 && run.spec.is_some() {
        behaviour_notes(f, &grade.behaviours)?;
    }

    // the completions
    for completion in &run.completions {
        writeln!(
            f,
            "- completion `{}` {} in {}s: {} input, {} cached, {} output ({} reasoning)",
            completion.label,
            completion.outcome,
            completion.duration.as_secs(),
            completion.tokens.input,
            completion.tokens.cached,
            completion.tokens.output,
            completion.tokens.reasoning,
        )?;
    }
    if let Some(build) = &run.build {
        build_notes(f, run, build)?;
    }

    Ok(())
}

// The requirements each source stated through a mechanism, with the
// identifiers each names, one line per source naming any.
fn mechanism_notes(
    f: &mut Formatter<'_>, run: &Run, sources: &[SourceGrade], led: &dyn Fn(&str) -> String,
) -> fmt::Result {
    let stated = run.mechanism();
    for source in sources {
        let listed: Vec<String> = stated
            .iter()
            .filter(|stated| stated.sources.contains(&source.name))
            .map(|stated| {
                let identifiers: Vec<String> =
                    stated.identifiers.iter().map(|name| format!("`{name}`")).collect();
                format!("{} ({})", stated.id, identifiers.join(", "))
            })
            .collect();
        if !listed.is_empty() {
            writeln!(
                f,
                "- {}mechanism {}/{}: {}",
                led(&source.name),
                listed.len(),
                run.citing(&source.name),
                listed.join(", ")
            )?;
        }
    }
    Ok(())
}

// The build beneath its run: how it ended, each wave's slices as they
// merged, what the model resolved, what the verify refused, what was left
// uncovered, the chain the repository holds, and the turns it spent.
fn build_notes(f: &mut Formatter<'_>, run: &Run, build: &Build) -> fmt::Result {
    let integrated = build.integrated();
    let tokens = build.tokens();
    build_ending(f, run, build, integrated.len())?;

    // each wave's slices, as they merged, and what its verify sealed
    let waves: BTreeSet<usize> = integrated.iter().map(|slice| slice.wave).collect();
    let turns = build.wave_turns();
    let repairs = build.repairs();
    for wave in waves {
        let slices: Vec<String> = integrated
            .iter()
            .filter(|slice| slice.wave == wave)
            .map(|slice| {
                let merged = slice.commit.as_deref().map_or_else(
                    || "nothing to merge".to_owned(),
                    |c| format!("merged {}", short(c)),
                );
                format!(
                    "{} {} (covered {}/{}, {} files, {merged})",
                    slice.id,
                    slice.name,
                    slice.covered.len(),
                    slice.requirements.len(),
                    slice.files.len()
                )
            })
            .collect();
        let spent = wave
            .checked_sub(1)
            .and_then(|k| turns.get(k))
            .map_or_else(String::new, |turns| turns.spent(repairs.get(&wave)));
        writeln!(f, "- wave {wave}: {}{spent}", slices.join(", "))?;
    }
    build_findings(f, build, &integrated, &repairs)?;

    // the chain, newest first
    if !build.history.is_empty() {
        let lines: Vec<String> = build
            .history
            .iter()
            .map(|landed| {
                let files = match landed.files.len() {
                    0 => String::new(),
                    n => format!(" ({n} files)"),
                };
                format!("{} {}{files}", short(&landed.commit), landed.subject)
            })
            .collect();
        writeln!(f, "- history: {}", lines.join(" · "))?;
    }

    // the turns
    writeln!(
        f,
        "- build turns: {} completions, {} input, {} cached, {} output ({} reasoning); serial {}s, \
         critical path {}s",
        build.completions.len(),
        tokens.input,
        tokens.cached,
        tokens.output,
        tokens.reasoning,
        build.serial().as_secs(),
        build.critical().as_secs(),
    )?;
    for completion in &build.completions {
        let writes = if completion.writes.any() {
            format!("; {}", completion.writes)
        } else {
            String::new()
        };
        writeln!(
            f,
            "- build completion `{}` {} in {}s: {} input, {} cached, {} output ({} \
             reasoning){writes}",
            completion.label,
            completion.outcome,
            completion.duration.as_secs(),
            completion.tokens.input,
            completion.tokens.cached,
            completion.tokens.output,
            completion.tokens.reasoning,
        )?;
    }

    Ok(())
}

// What the build's waves left to read: the slices whose merge the model
// resolved and the paths, the waves the verify repaired and the files each
// sealed, the verify's refusals, the uncovered ids per slice, and what the
// host's checks say of the tree.
fn build_findings(
    f: &mut Formatter<'_>, build: &Build, integrated: &[Integrated],
    repairs: &BTreeMap<usize, Vec<String>>,
) -> fmt::Result {
    for slice in integrated.iter().filter(|slice| !slice.resolved.is_empty()) {
        writeln!(
            f,
            "- resolved: {} at {}, combined by the model and merged again",
            slice.id,
            slice.resolved.join(", ")
        )?;
    }
    let repaired = build.repaired();
    if !repaired.is_empty() {
        let waves: Vec<String> = repaired
            .iter()
            .map(|wave| match repairs.get(wave) {
                Some(files) if files.is_empty() => format!("wave {wave} (lock only)"),
                Some(files) => format!("wave {wave} ({})", files.join(", ")),
                None => format!("wave {wave}"),
            })
            .collect();
        writeln!(f, "- repaired by the verify: {}", waves.join(", "))?;
    }
    for failure in build.verify_failures() {
        writeln!(f, "- verify refused: {failure}")?;
    }
    for slice in integrated.iter().filter(|slice| !slice.uncovered().is_empty()) {
        writeln!(f, "- uncovered: {} {}", slice.id, slice.uncovered().join(", "))?;
    }
    if let Some(fidelity) = &build.fidelity {
        let agree = match (build.landed(), fidelity.clean()) {
            (true, true) => "the host's checks agree with the verdicts",
            (true, false) => "the host's checks disagree with the passed verdicts",
            (false, true) => "the host's checks pass the tree the run left",
            (false, false) => "the host's checks fail the tree the run left",
        };
        writeln!(
            f,
            "- host checks over {}: {}; scenarios named {}; {agree} (`{}`)",
            fidelity.tree,
            fidelity.host_cell(),
            fidelity.scenarios_cell(),
            fidelity.file,
        )?;
        if let Some(comparison) = &fidelity.reference {
            writeln!(f, "- {} (`{}`)", comparison.finding(), fidelity.file)?;
        }
    }
    Ok(())
}

// How the build ended: where the label stands, or the failure's code, exit,
// and first line; then, for a build that resumed, where it resumed from.
fn build_ending(f: &mut Formatter<'_>, run: &Run, build: &Build, merged: usize) -> fmt::Result {
    let exit = build.exit.map_or_else(|| "killed".to_owned(), |code| format!("exit {code}"));
    match (&build.failure, build.exit) {
        (_, Some(0)) => {
            let envelope = build.envelope.as_ref();
            let label = envelope.and_then(|out| out["label"].as_str()).unwrap_or("");
            let head = envelope.and_then(|out| out["head"].as_str()).unwrap_or("");
            let resumed = if build.rewound.is_some() {
                build.resumption()
            } else {
                envelope
                    .and_then(|out| out["resumed"].as_array())
                    .filter(|resumed| !resumed.is_empty())
                    .map_or_else(String::new, |resumed| format!(", {} resumed", resumed.len()))
            };
            writeln!(
                f,
                "\nBuild: {merged}/{} slices merged in {} waves in {}s{resumed}; `{label}` at `{}`",
                run.slices(),
                build.waves(),
                build.wall.as_secs(),
                short(head),
            )?;
        }
        (Some(failure), _) => {
            let first = failure.message.lines().next().unwrap_or("");
            writeln!(
                f,
                "\nBuild failed `{}` ({exit}) after {}s, {merged}/{} slices merged{}: {first}",
                failure.code,
                build.wall.as_secs(),
                run.slices(),
                build.resumption(),
            )?;
        }
        (None, _) => writeln!(
            f,
            "\nBuild failed ({exit}) after {}s with no envelope{}:\n\n```\n{}\n```",
            build.wall.as_secs(),
            build.resumption(),
            build.tail
        )?,
    }
    if let Some(rewound) = &build.rewound {
        let slices = if rewound.slices.is_empty() {
            "no slice".to_owned()
        } else {
            rewound.slices.join(", ")
        };
        writeln!(
            f,
            "\nResumed from `{}` at wave {}: the label moved to `{}` ({}), the chain naming \
             {slices}; built this run: {}",
            rewound.card,
            rewound.wave,
            short(&rewound.commit),
            rewound.subject,
            build.built().iter().map(|slice| slice.id.as_str()).collect::<Vec<_>>().join(", ")
        )?;
    }
    Ok(())
}

// A commit as the card spells it: its first eight characters.
pub fn short(commit: &str) -> &str {
    commit.get(..8).unwrap_or(commit)
}

// One source's survey: the surfaces its adapter logged, what it placed
// nowhere, and how the (entry, stem) pairs and the stems read against the
// case; `led` names the source where the case has several.
fn survey_notes(
    f: &mut Formatter<'_>, run: &Run, source: &SourceGrade, led: &str, graded: bool,
) -> fmt::Result {
    if let Some(surveyed) = run.surveyed.get(&source.name) {
        let listed: Vec<String> = surveyed
            .iter()
            .map(|surface| {
                let ids: Vec<String> = surface.ids.iter().map(|id| format!("`{id}`")).collect();
                let ids = if ids.is_empty() {
                    String::new()
                } else {
                    format!(" (id {})", ids.join(", "))
                };
                format!("`{}` @ `{}` as `{}`{ids}", surface.name, surface.entry, surface.stem)
            })
            .collect();
        writeln!(f, "- {led}surfaces: {}", listed.join(", "))?;
    }
    if let Some(unreached) = run.unreached.get(&source.name) {
        writeln!(f, "- {led}unreached: {unreached} modules the survey placed under no surface")?;
    }
    if source.surfaces.surveyed {
        for restemmed in &source.surfaces.restemmed {
            writeln!(f, "- {led}surface restemmed: {restemmed}")?;
        }
        for missed in &source.surfaces.missed {
            writeln!(f, "- {led}surface missed: {missed}")?;
        }
        if source.surfaces.expected > 0 {
            for extra in &source.surfaces.extra {
                writeln!(f, "- {led}surface extra: {extra}")?;
            }
        }
    } else if source.surfaces.expected > 0 {
        writeln!(f, "- {led}no survey: the adapter cut its seams without deciding a surface")?;
    }
    if graded {
        if !source.missing_stems.is_empty() {
            writeln!(f, "- {led}stems missing: {:?}", source.missing_stems)?;
        }
        if !source.extra_stems.is_empty() {
            writeln!(f, "- {led}stems extra: {:?}", source.extra_stems)?;
        }
    }

    Ok(())
}

// The reconciled requirements against the behaviours expected.
fn behaviour_notes(f: &mut Formatter<'_>, behaviours: &Behaviours) -> fmt::Result {
    if behaviours.status_expected > 0 {
        writeln!(
            f,
            "- behaviour statuses: {}/{} as expected",
            behaviours.status_met, behaviours.status_expected
        )?;
    }
    for (what, listed) in [
        ("missed", &behaviours.missed),
        ("split", &behaviours.split),
        ("partial", &behaviours.partial),
        ("met by criterion", &behaviours.by_criterion),
        ("across kinds", &behaviours.across),
        ("status", &behaviours.status_wrong),
        ("wrong merge", &behaviours.wrong),
    ] {
        for item in listed {
            writeln!(f, "- behaviour {what}: {item}")?;
        }
    }
    if behaviours.premerged.is_empty() {
        writeln!(f, "- cross-source pre-merges: none")
    } else {
        writeln!(
            f,
            "- cross-source pre-merges: {} — {}",
            behaviours.premerged.len(),
            behaviours.premerged.join("; ")
        )
    }
}

// One row per run: pass or fail at each rung attempted, `—` past the rung
// it landed on.
fn ladder(
    f: &mut Formatter<'_>, runs: &[Vec<Attempt>], ladder: &[Rung], backend: Backend,
) -> fmt::Result {
    let (timeout, inactivity) = backend.rung();
    writeln!(
        f,
        "\nBudget ladder (`{timeout}`/`{inactivity}`), climbed on a `timeout` or `inactive` \
         completion:\n"
    )?;
    let heads: Vec<String> = ladder.iter().map(Rung::to_string).collect();
    writeln!(f, "| run | {} |", heads.join(" | "))?;
    writeln!(f, "|{}", " --- |".repeat(ladder.len() + 1))?;
    for attempts in runs {
        let Some(first) = attempts.first() else { continue };
        let cells: Vec<String> = (0..ladder.len())
            .map(|index| attempts.get(index).map_or_else(|| "—".to_owned(), verdict))
            .collect();
        writeln!(f, "| {} | {} |", first.run.n, cells.join(" | "))?;
    }

    Ok(())
}

fn verdict(attempt: &Attempt) -> String {
    let Attempt { run, grade } = attempt;
    let wall = run.wall.as_secs();
    if run.passed() {
        return format!(
            "pass (req {}/{}, {wall}s)",
            grade.requirements.matched, grade.requirements.expected
        );
    }
    let starved: Vec<String> = ["timeout", "inactive"]
        .into_iter()
        .filter_map(|outcome| {
            let count = run.completions.iter().filter(|c| c.outcome == outcome).count();
            (count > 0).then(|| format!("{count} {outcome}"))
        })
        .collect();
    let why = if starved.is_empty() {
        let failed: BTreeSet<&str> = run
            .completions
            .iter()
            .filter(|c| !matches!(c.outcome.as_str(), "ok" | "corrected" | "abort"))
            .map(|c| c.outcome.as_str())
            .collect();
        let died = run.died_in().map_or(String::new(), |label| format!(" in `{label}`"));
        match (run.exit, failed.is_empty()) {
            (Some(code), true) => format!("exit {code}{died}"),
            (Some(code), false) => {
                format!("exit {code}, {}{died}", Vec::from_iter(failed).join(", "))
            }
            (None, _) => "killed".to_owned(),
        }
    } else {
        starved.join(", ")
    };

    format!("fail: {why} ({wall}s)")
}

fn stability(f: &mut Formatter<'_>, label: &str, attempts: &[&Attempt]) -> fmt::Result {
    writeln!(f, "\nStability {label}:")?;
    let runs: Vec<&Run> = attempts.iter().map(|attempt| &attempt.run).collect();
    let runs = runs.as_slice();

    // the stems each run's requirements lead with, as sets of (source, stem)
    let sets: BTreeSet<BTreeSet<(&str, &str)>> = runs.iter().map(|run| run.stems()).collect();
    if sets.len() == 1 {
        writeln!(f, "- stem sets: 1 distinct")?;
    } else {
        writeln!(f, "- stem sets: {} distinct — {:?}", sets.len(), sets)?;
    }

    // every pair of runs
    pairwise(f, "surfaces (entry, stem)", runs, Run::pairs)?;
    pairwise(f, "requirement ids", runs, Run::ids)?;
    pairwise(f, "spec subjects", runs, Run::subjects)?;
    anchored(f, runs)?;

    // the band the absent hits fell in, where the case expects any
    if attempts.iter().any(|attempt| attempt.grade.absent.expected > 0) {
        let hits: Vec<usize> =
            attempts.iter().map(|attempt| attempt.grade.absent.hits.len()).collect();
        let (low, high) = (hits.iter().min().copied(), hits.iter().max().copied());
        let per_run: Vec<String> = attempts
            .iter()
            .zip(&hits)
            .map(|(attempt, hits)| format!("run {} {hits}", attempt.run.n))
            .collect();
        if let (Some(low), Some(high)) = (low, high) {
            writeln!(f, "- absent hits: {low}–{high} ({})", per_run.join(" · "))?;
        }
    }

    Ok(())
}

// The Jaccard index of `of` over every pair of runs, as one line.
fn pairwise<'r, T: Ord>(
    f: &mut Formatter<'_>, label: &str, runs: &[&'r Run], of: impl Fn(&'r Run) -> BTreeSet<T>,
) -> fmt::Result {
    let sets: Vec<BTreeSet<T>> = runs.iter().map(|run| of(run)).collect();
    let mut cells = Vec::new();
    for (left, first) in sets.iter().enumerate() {
        for (right, second) in sets.iter().enumerate().skip(left + 1) {
            cells.push(format!(
                "{}↔{} {}",
                runs[left].n,
                runs[right].n,
                Jaccard::of(first, second)
            ));
        }
    }
    writeln!(f, "- {label}: {}", cells.join(" · "))
}

// The requirements every pair of runs anchors alike, as one line in the
// `pairwise` shape: the pairs the engine's re-mine diff would match — the
// same stem and a cited `path` in common, one to one, the pair sharing the
// most anchors first — over the requirements the two revisions hold together.
// Reads the `sources[].path` that `emery show --format json` carries from
// grammar `4`; a revision stored under an earlier grammar matches none.
fn anchored(f: &mut Formatter<'_>, runs: &[&Run]) -> fmt::Result {
    let mut cells = Vec::new();
    for (left, first) in runs.iter().enumerate() {
        for second in runs.iter().skip(left + 1) {
            let shared = matched(first.requirements(), second.requirements());
            let union = first.requirements().len() + second.requirements().len() - shared;
            cells.push(format!("{}↔{} {}", first.n, second.n, Jaccard { shared, union }));
        }
    }
    writeln!(f, "- anchor-matched requirements: {}", cells.join(" · "))
}

// How many one-to-one pairs of `outgoing` and `incoming` requirements share a
// stem and cite an anchor in common: every pair scored by the anchors it
// shares, the highest taken first, ties in position order — the engine's
// pairing, less its fallback to id.
fn matched(outgoing: &[Value], incoming: &[Value]) -> usize {
    let mut scored: Vec<(usize, usize, usize)> = Vec::new();
    for (out, before) in outgoing.iter().enumerate() {
        let Some(under) = before["subject"].as_str().and_then(stem) else { continue };
        for (into, after) in incoming.iter().enumerate() {
            if after["subject"].as_str().and_then(stem) != Some(under) {
                continue;
            }
            let shared = citations(before)
                .flat_map(|cited| citations(after).filter(move |other| cited_alike(cited, *other)))
                .count();
            if shared > 0 {
                scored.push((shared, out, into));
            }
        }
    }
    scored.sort_by_key(|&(shared, out, into)| (Reverse(shared), out, into));
    let mut outs = BTreeSet::new();
    let mut intos = BTreeSet::new();
    for (_, out, into) in scored {
        if !outs.contains(&out) && !intos.contains(&into) {
            outs.insert(out);
            intos.insert(into);
        }
    }
    outs.len()
}

struct Jaccard {
    shared: usize,
    union: usize,
}

impl Jaccard {
    fn of<T: Ord>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> Self {
        Self {
            shared: a.intersection(b).count(),
            union: a.union(b).count(),
        }
    }
}

impl Display for Jaccard {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.union == 0 {
            return f.write_str("—");
        }
        write!(f, "{}/{} ({}%)", self.shared, self.union, self.shared * 100 / self.union)
    }
}

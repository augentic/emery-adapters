//! Renders the evolution scorecard and `## Evolution` section: step rows, checks `C1`–`C6`,
//! the six diagnostic measures, and per-step cost.

use std::fmt::{self, Display, Formatter, Write as _};
use std::iter::Sum;
use std::ops::{AddAssign, RangeInclusive};
use std::time::Duration;

use super::{
    adapters_summary, build_flags_summary, comma_joined, knobs_summary, ladder_summary, short,
};
use crate::Settings;
use crate::evolve::{ChainRun, EvolutionReport, OraclePartition, StepResult, step_tag};
use crate::locality::{AmendGrade, RoundTripDelta, ScopeGrade, StepLocality};
use crate::run::Tokens;

pub fn evolution_scorecard(
    settings: &Settings, reports: &[EvolutionReport], started: &str, name: &str,
) -> String {
    let (timeout, inactivity) = settings.backend.rung();
    let mut card = format!(
        "# Eval {started}\n\nmodel `{}` (`{}`) · backend {} · emery `{}` · {} · ladder {} \
         (`{timeout}`/`{inactivity}`){} · build under {} at {} \
         (`EVAL_BUILD_RUNG`) · runs under `target/eval/{name}/`\n",
        settings.model,
        settings.backend.model().0,
        settings.backend.name(),
        settings.emery.display(),
        adapters_summary(settings),
        ladder_summary(settings),
        knobs_summary(settings),
        build_flags_summary(settings),
        settings.build_rung,
    );
    let _ = evolution_section(&mut card, reports);
    card
}

pub fn evolution_section(f: &mut impl fmt::Write, reports: &[EvolutionReport]) -> fmt::Result {
    if reports.is_empty() {
        return Ok(());
    }
    writeln!(f, "\n## Evolution")?;
    for report in reports {
        write!(f, "{report}")?;
    }
    Ok(())
}

impl Display for EvolutionReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\nCase `{}` · base `{}` · target `{}` · {} steps",
            self.evolution.name,
            self.evolution.base.name,
            self.target.name(),
            self.evolution.steps.len(),
        )?;
        if let Some(range) = &self.step_range {
            write!(f, " · steps `{range}`")?;
        }
        if let Some(resumed) = &self.resumed {
            write!(
                f,
                " · resumed from `{}` chain {} at `{}`",
                resumed.card,
                resumed.chain,
                step_tag(resumed.at_step),
            )?;
        }
        writeln!(f)?;

        let window = StepWindow::from_report(self);
        for chain in &self.chains {
            write_chain(f, chain, window)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckId {
    C1,
    C2,
    C3,
    C4,
    C5,
    C6,
}

impl Display for CheckId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::C1 => "C1",
            Self::C2 => "C2",
            Self::C3 => "C3",
            Self::C4 => "C4",
            Self::C5 => "C5",
            Self::C6 => "C6",
        };
        f.write_str(code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckVerdict {
    Pass,
    Fail,
    Skipped,
}

impl Display for CheckVerdict {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Pass => "pass",
            Self::Fail => "FAIL",
            Self::Skipped => "—",
        };
        f.write_str(label)
    }
}

struct Check {
    id: CheckId,
    verdict: CheckVerdict,
    reading: String,
    evidence: String,
}

impl Check {
    const fn pass(id: CheckId, reading: String, evidence: String) -> Self {
        Self {
            id,
            verdict: CheckVerdict::Pass,
            reading,
            evidence,
        }
    }

    const fn fail(id: CheckId, reading: String, evidence: String) -> Self {
        Self {
            id,
            verdict: CheckVerdict::Fail,
            reading,
            evidence,
        }
    }

    fn skipped(id: CheckId, reading: impl Into<String>) -> Self {
        Self {
            id,
            verdict: CheckVerdict::Skipped,
            reading: reading.into(),
            evidence: "—".to_owned(),
        }
    }
}

impl Display for Check {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "| `{}` | {} | {} | {} |", self.id, self.verdict, self.reading, self.evidence)
    }
}

struct Checks([Check; 6]);

impl Checks {
    fn evaluate(chain: &ChainRun, window: StepWindow) -> Self {
        Self([
            check_c1(chain, window),
            check_c2(chain, window),
            check_c3(chain, window),
            check_c4(chain, window),
            check_c5(chain, window),
            check_c6(chain, window),
        ])
    }

    fn badge(&self) -> String {
        let passed = self.0.iter().filter(|check| check.verdict == CheckVerdict::Pass).count();
        let failed = self.0.iter().filter(|check| check.verdict == CheckVerdict::Fail).count();
        let evaluated = passed + failed;
        if failed == 0 && evaluated == self.0.len() {
            format!("pass ({passed}/{evaluated})")
        } else if failed == 0 {
            format!("partial ({passed}/{evaluated} evaluated)")
        } else {
            format!("FAIL ({passed}/{evaluated})")
        }
    }
}

impl Display for Checks {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\n#### Checks\n\n\
             | check | verdict | reading | evidence |\n\
             | --- | --- | --- | --- |"
        )?;
        for check in &self.0 {
            writeln!(f, "{check}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct StepWindow {
    total_steps: usize,
    first_executed: usize,
    last_executed: usize,
}

impl StepWindow {
    fn from_report(report: &EvolutionReport) -> Self {
        let total_steps = report.evolution.steps.len();
        let first_executed = report.resumed.as_ref().map_or(0, |resumed| resumed.at_step);
        let last_executed = report.step_range.map_or(total_steps, |range| range.end);
        Self {
            total_steps,
            first_executed,
            last_executed,
        }
    }

    const fn first_amend(self) -> usize {
        if self.first_executed == 0 { 1 } else { self.first_executed }
    }

    const fn is_full(self) -> bool {
        self.first_executed == 0 && self.last_executed == self.total_steps
    }

    fn amend_range(self) -> Option<RangeInclusive<usize>> {
        let first = self.first_amend();
        (first <= self.last_executed).then_some(first..=self.last_executed)
    }

    fn amend_count(self) -> usize {
        self.amend_range().map_or(0, Iterator::count)
    }

    fn amend_span(self) -> String {
        format!("`{}`..`{}`", step_tag(self.first_amend()), step_tag(self.last_executed))
    }

    // Resolves a single target amend step (`step-05`, `step-06`, or final `step-07`)
    // against `--steps` and `--resume --at-step`, returning its `StepLocality` or
    // the `Skipped` / `Fail` check explaining why it was not reached.
    fn require_locality(
        self, chain: &ChainRun, id: CheckId, target_step: usize,
    ) -> Result<&StepLocality, Check> {
        let tag = step_tag(target_step);
        if target_step > self.total_steps || target_step > self.last_executed {
            return Err(Check::skipped(id, format!("`{tag}` outside `--steps` range")));
        }
        if target_step < self.first_amend() {
            return Err(Check::skipped(
                id,
                format!("`{tag}` resumed before `--at-step {}`", self.first_amend()),
            ));
        }
        let Some(step) = chain.step(target_step) else {
            return Err(Check::fail(
                id,
                format!("chain stopped before `{tag}`"),
                chain.stopped_evidence(),
            ));
        };
        step.locality.as_ref().ok_or_else(|| {
            Check::fail(id, format!("`{tag}` did not land"), format!("`{tag}.build.stderr`"))
        })
    }
}

struct StepRow {
    tag: String,
    revision: String,
    title: String,
    amend: String,
    scope: String,
    carried: String,
    touch: String,
    oracle_carried: String,
    oracle_step: String,
    orders_loc: String,
    turns: String,
    wall: String,
}

impl From<&StepResult> for StepRow {
    fn from(step: &StepResult) -> Self {
        let revision = step
            .revision
            .as_deref()
            .map_or_else(|| "—".to_owned(), |rev| format!("`{}`", short(rev)));
        if step.resumed {
            return Self {
                tag: step.tag.clone(),
                revision,
                title: step.title.clone(),
                amend: "resumed".to_owned(),
                scope: "resumed".to_owned(),
                carried: "—".to_owned(),
                touch: "—".to_owned(),
                oracle_carried: "—".to_owned(),
                oracle_step: "—".to_owned(),
                orders_loc: "—".to_owned(),
                turns: "—".to_owned(),
                wall: "—".to_owned(),
            };
        }
        let (oracle_carried, oracle_step) = oracle_cells(step);
        let orders_loc = step
            .locality
            .as_ref()
            .and_then(|locality| locality.sizes.get("orders"))
            .map_or_else(|| "—".to_owned(), |size| size.loc.to_string());
        Self {
            tag: step.tag.clone(),
            revision,
            title: step.title.clone(),
            amend: amend_cell(step),
            scope: scope_cell(step),
            carried: carried_cell(step),
            touch: touch_cell(step),
            oracle_carried,
            oracle_step,
            orders_loc,
            turns: step.turns().to_string(),
            wall: format!("{}s", step.wall().as_secs()),
        }
    }
}

impl Display for StepRow {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            self.tag,
            self.revision,
            self.title,
            self.amend,
            self.scope,
            self.carried,
            self.touch,
            self.oracle_carried,
            self.oracle_step,
            self.orders_loc,
            self.turns,
            self.wall,
        )
    }
}

#[derive(Default)]
struct StepCost {
    stage_turns: usize,
    stage_wall: Duration,
    slice_turns: usize,
    verify_turns: usize,
    corrected: usize,
    repairs: usize,
    edits: usize,
    writes: usize,
    refused: usize,
    shells: usize,
    tokens: Tokens,
    wall: Duration,
}

impl From<&StepResult> for StepCost {
    fn from(step: &StepResult) -> Self {
        let build_completions = step.build.iter().flat_map(|build| &build.completions);
        let slice_turns =
            build_completions.clone().filter(|c| c.label.starts_with("build-")).count();
        let verify_turns = build_completions.clone().filter(|c| c.label == "verify").count();
        let mut cost = Self {
            stage_turns: step.amend_completions.len(),
            stage_wall: step.amend_wall,
            slice_turns,
            verify_turns,
            repairs: step.build.as_ref().map_or(0, |build| build.repaired().len()),
            wall: step.wall(),
            ..Self::default()
        };
        for completion in step.amend_completions.iter().chain(build_completions) {
            cost.corrected += usize::from(completion.outcome == "corrected");
            cost.edits += completion.writes.edits;
            cost.writes += completion.writes.calls;
            cost.refused += completion.writes.refused;
            cost.shells += completion.shell.calls;
            cost.tokens = cost.tokens.add(completion.tokens);
        }
        cost
    }
}

impl StepCost {
    const fn tool_calls(&self) -> usize {
        self.edits + self.writes + self.refused + self.shells
    }

    const fn turns(&self) -> usize {
        self.stage_turns + self.slice_turns + self.verify_turns
    }

    const fn build_wall(&self) -> Duration {
        self.wall.saturating_sub(self.stage_wall)
    }
}

impl AddAssign for StepCost {
    fn add_assign(&mut self, other: Self) {
        self.stage_turns += other.stage_turns;
        self.stage_wall = self.stage_wall.saturating_add(other.stage_wall);
        self.slice_turns += other.slice_turns;
        self.verify_turns += other.verify_turns;
        self.corrected += other.corrected;
        self.repairs += other.repairs;
        self.edits += other.edits;
        self.writes += other.writes;
        self.refused += other.refused;
        self.shells += other.shells;
        self.tokens = self.tokens.add(other.tokens);
        self.wall = self.wall.saturating_add(other.wall);
    }
}

impl Sum for StepCost {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut total = Self::default();
        for cost in iter {
            total += cost;
        }
        total
    }
}

fn write_chain(f: &mut Formatter<'_>, chain: &ChainRun, window: StepWindow) -> fmt::Result {
    let checks = Checks::evaluate(chain, window);
    writeln!(f, "\n### Chain {} — {}\n", chain.n, checks.badge())?;
    write_steps_table(f, &chain.steps)?;
    write!(f, "{checks}")?;
    write_measures(f, chain, window)?;
    write_cost_table(f, chain)
}

fn write_steps_table(f: &mut Formatter<'_>, steps: &[StepResult]) -> fmt::Result {
    writeln!(
        f,
        "| step | revision | title | amend | scope | carried id | touch | oracle carried | oracle step | orders LOC | turns | wall |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )?;
    for step in steps {
        writeln!(f, "{}", StepRow::from(step))?;
    }
    Ok(())
}

fn amend_cell(step: &StepResult) -> String {
    if step.is_baseline() {
        return match step.amend_exit {
            Some(0) => "specify".to_owned(),
            Some(code) => format!("specify exit {code} FAIL"),
            None => "specify killed FAIL".to_owned(),
        };
    }
    if let Some(amend) = step.locality.as_ref().and_then(|locality| locality.amend.as_ref()) {
        return summarize_amend(amend);
    }
    match step.amend_exit {
        Some(0) => "staged (build failed)".to_owned(),
        Some(code) => format!("exit {code} FAIL"),
        None => "killed FAIL".to_owned(),
    }
}

fn summarize_amend(amend: &AmendGrade) -> String {
    let mut text = format!("+{} ~{} -{}", amend.added, amend.changed, amend.retired);
    if !amend.stems.is_empty() {
        let _ = write!(text, " [{}]", comma_joined(&amend.stems));
    }
    if !amend.sections.is_empty() {
        let _ = write!(text, " sections [{}]", comma_joined(&amend.sections));
    }
    if !amend.parts_added.is_empty() {
        let _ = write!(text, " +parts [{}]", comma_joined(&amend.parts_added));
    }
    if !amend.parts_removed.is_empty() {
        let _ = write!(text, " -parts [{}]", comma_joined(&amend.parts_removed));
    }
    text.push_str(if amend.passed() { " ok" } else { " FAIL" });
    text
}

fn scope_cell(step: &StepResult) -> String {
    if step.is_baseline() {
        let Some(build) = &step.build else {
            return "—".to_owned();
        };
        if !build.landed() {
            return "whole FAIL".to_owned();
        }
        let reason = build
            .envelope
            .as_ref()
            .and_then(|env| env["whole"]["reason"].as_str())
            .unwrap_or("no-parent");
        return format!("whole ({reason})");
    }
    step.locality
        .as_ref()
        .and_then(|locality| locality.scope.as_ref())
        .map_or_else(|| "—".to_owned(), summarize_scope)
}

fn summarize_scope(scope: &ScopeGrade) -> String {
    if let Some(reason) = &scope.whole_reason {
        return format!("whole ({reason}) FAIL");
    }
    let rebuilt: Vec<String> =
        scope
            .rebuilt
            .iter()
            .map(|part| {
                if scope.widened.contains(part) {
                    format!("{part} (widened)")
                } else {
                    part.clone()
                }
            })
            .collect();
    let mut text = format!("rebuilt [{}]", rebuilt.join(", "));
    if !scope.retiring.is_empty() {
        let _ = write!(text, "; retiring [{}]", comma_joined(&scope.retiring));
    }
    let _ = write!(text, "; carried [{}]", comma_joined(&scope.carried));
    text.push_str(if scope.passed() { " ok" } else { " FAIL" });
    text
}

fn carried_cell(step: &StepResult) -> String {
    if step.is_baseline() {
        return "—".to_owned();
    }
    step.locality
        .as_ref()
        .map_or("—", |locality| if locality.carried_byte_identical() { "yes" } else { "NO" })
        .to_owned()
}

fn touch_cell(step: &StepResult) -> String {
    if step.is_baseline() {
        return "—".to_owned();
    }
    let Some(locality) = &step.locality else {
        return "—".to_owned();
    };
    let violations = locality.touch_violations().len();
    let admitted = admitted_touches(locality).count();
    if violations > 0 {
        format!("{violations} FAIL")
    } else if admitted > 0 {
        format!("0 ({admitted} ok)")
    } else {
        "0".to_owned()
    }
}

fn oracle_cells(step: &StepResult) -> (String, String) {
    let Some(oracle) = &step.oracle else {
        return ("—".to_owned(), "—".to_owned());
    };
    if step.is_baseline() {
        let own = if oracle.baseline_ran() {
            format!("{}/{}", oracle.step_passed, oracle.step_total)
        } else {
            format!("{}/{} FAIL", oracle.step_passed, oracle.step_total)
        };
        return ("—".to_owned(), own);
    }
    let carried = if oracle.carried_clean() {
        format!("{}/{}", oracle.carried_passed, oracle.carried_total)
    } else if !oracle.regressed.is_empty() {
        format!(
            "{}/{} (-{} FAIL)",
            oracle.carried_passed,
            oracle.carried_total,
            oracle.regressed.len()
        )
    } else {
        format!("{}/{} FAIL", oracle.carried_passed, oracle.carried_total)
    };
    let own = if oracle.step_clean() {
        format!("{}/{}", oracle.step_passed, oracle.step_total)
    } else {
        format!("{}/{} FAIL", oracle.step_passed, oracle.step_total)
    };
    (carried, own)
}

fn check_c1(chain: &ChainRun, window: StepWindow) -> Check {
    let Some(range) = window.amend_range() else {
        return Check::skipped(CheckId::C1, "no amend steps in `--steps 00`");
    };
    let first = *range.start();
    let last = *range.end();
    let expected = range.clone().count();
    let mut clean_steps = 0;
    let mut issues = Vec::new();
    let mut issue_files = Vec::new();
    for step_num in range {
        let tag = step_tag(step_num);
        let Some(step) = chain.step(step_num) else {
            issues.push(format!("chain stopped before `{tag}`"));
            break;
        };
        if let Some(locality) = &step.locality {
            let violations = locality.touch_violations();
            if locality.carried_byte_identical() && violations.is_empty() {
                clean_steps += 1;
            } else {
                if !locality.carried_byte_identical() {
                    issues.push(format!("`{tag}` carried crate moved"));
                }
                if !violations.is_empty() {
                    issues.push(format!("`{tag}` {} touch violation(s)", violations.len()));
                }
                issue_files.push(format!("`{tag}.locality.md`"));
            }
        } else {
            issues.push(format!("`{tag}` did not land"));
            issue_files.push(format!("`{tag}.build.stderr`"));
        }
    }
    if issues.is_empty() {
        let reading = if window.is_full() {
            format!("{clean_steps}/{expected} steps carried byte-identical, 0 touch violations")
        } else {
            format!(
                "{clean_steps}/{expected} steps in {} carried byte-identical, 0 touch violations",
                window.amend_span()
            )
        };
        Check::pass(
            CheckId::C1,
            reading,
            format!("`{}.locality.md` .. `{}.locality.md`", step_tag(first), step_tag(last)),
        )
    } else {
        let evidence =
            if issue_files.is_empty() { chain.stopped_evidence() } else { issue_files.join(", ") };
        Check::fail(
            CheckId::C1,
            format!("{clean_steps}/{expected} clean; {}", issues.join(", ")),
            evidence,
        )
    }
}

fn check_c2(chain: &ChainRun, window: StepWindow) -> Check {
    let Some(range) = window.amend_range() else {
        return Check::skipped(CheckId::C2, "no amend steps in `--steps 00`");
    };
    check_oracle_range(chain, CheckId::C2, range, "steps carried scenarios pass", |oracle| {
        let detail = if oracle.regressed.is_empty() {
            format!("{}/{}", oracle.carried_passed, oracle.carried_total)
        } else {
            format!(
                "{}/{} (-{}: {})",
                oracle.carried_passed,
                oracle.carried_total,
                oracle.regressed.len(),
                comma_joined(&oracle.regressed),
            )
        };
        (oracle.carried_clean(), detail)
    })
}

fn check_c3(chain: &ChainRun, window: StepWindow) -> Check {
    let Some(range) = window.amend_range() else {
        return Check::skipped(CheckId::C3, "no amend steps in `--steps 00`");
    };
    check_oracle_range(chain, CheckId::C3, range, "steps pass own scenarios", |oracle| {
        (oracle.step_clean(), format!("{}/{}", oracle.step_passed, oracle.step_total))
    })
}

fn check_oracle_range(
    chain: &ChainRun, id: CheckId, range: RangeInclusive<usize>, noun: &str,
    select: impl Fn(&OraclePartition) -> (bool, String),
) -> Check {
    let first = *range.start();
    let last = *range.end();
    let expected = range.clone().count();
    let mut counts = Vec::new();
    let mut issues = Vec::new();
    let mut issue_files = Vec::new();
    for step_num in range {
        let tag = step_tag(step_num);
        let Some(step) = chain.step(step_num) else {
            issues.push(format!("chain stopped before `{tag}`"));
            break;
        };
        if let Some(oracle) = &step.oracle {
            let (clean, detail) = select(oracle);
            if clean {
                counts.push(detail);
            } else {
                issues.push(format!("`{tag}` {detail}"));
                issue_files.push(format!("`{tag}.oracle.log`"));
            }
        } else {
            issues.push(format!("`{tag}` oracle not run"));
            issue_files.push(format!("`{tag}.build.stderr`"));
        }
    }
    if issues.is_empty() {
        Check::pass(
            id,
            format!("{}/{expected} {noun} ({})", counts.len(), counts.join(", ")),
            format!("`{}.oracle.log` .. `{}.oracle.log`", step_tag(first), step_tag(last)),
        )
    } else {
        let evidence =
            if issue_files.is_empty() { chain.stopped_evidence() } else { issue_files.join(", ") };
        Check::fail(
            id,
            format!("{}/{expected} clean; {}", counts.len(), issues.join(", ")),
            evidence,
        )
    }
}

fn check_c4(chain: &ChainRun, window: StepWindow) -> Check {
    let target_step = window.total_steps;
    let locality = match window.require_locality(chain, CheckId::C4, target_step) {
        Ok(locality) => locality,
        Err(check) => return check,
    };
    let tag = step_tag(target_step);
    let vs_00 =
        locality.round_trips.iter().find(|rt| rt.against_step == 0 && rt.crate_dir == "orders");
    let vs_04 =
        locality.round_trips.iter().find(|rt| rt.against_step == 4 && rt.crate_dir == "orders");
    let Some(vs_00) = vs_00 else {
        return Check::fail(
            CheckId::C4,
            format!("`crates/orders` missing at `{tag}` or `step-00`"),
            format!("`{tag}.locality.md`"),
        );
    };
    let mut reading = format!("`crates/orders` at `{tag}`: {}", round_trip_summary(vs_00));
    if let Some(vs_04) = vs_04 {
        let _ = write!(reading, "; {}", round_trip_summary(vs_04));
    }
    let evidence = format!("`{tag}.locality.md`");
    if vs_00.loc_within_ten_percent() {
        Check::pass(CheckId::C4, reading, evidence)
    } else {
        Check::fail(CheckId::C4, reading, evidence)
    }
}

fn check_c5(chain: &ChainRun, window: StepWindow) -> Check {
    const TARGET_STEP: usize = 6;
    let locality = match window.require_locality(chain, CheckId::C5, TARGET_STEP) {
        Ok(locality) => locality,
        Err(check) => return check,
    };
    let tag = step_tag(TARGET_STEP);
    let Some(scope) = &locality.scope else {
        return Check::fail(
            CheckId::C5,
            format!("`{tag}` did not land"),
            format!("`{tag}.build.stderr`"),
        );
    };
    let evidence = format!("`{tag}.build.stdout`, `{tag}.locality.md`");
    if let Some(reason) = &scope.whole_reason {
        return Check::fail(
            CheckId::C5,
            format!("`{tag}` ran as whole (`{reason}`), expected `rebuilt [orders]`"),
            evidence,
        );
    }
    let reading = format!(
        "`{tag}` rebuilt `[{}]` (minimum `[{}]`, widened `[{}]`, retiring `[{}]`)",
        comma_joined(&scope.rebuilt),
        comma_joined(&scope.minimum),
        comma_joined(&scope.widened),
        comma_joined(&scope.retiring),
    );
    let exact_orders =
        scope.rebuilt.len() == 1 && scope.rebuilt.contains("orders") && scope.retiring.is_empty();
    if exact_orders {
        Check::pass(CheckId::C5, reading, evidence)
    } else {
        Check::fail(CheckId::C5, reading, evidence)
    }
}

fn check_c6(chain: &ChainRun, window: StepWindow) -> Check {
    const TARGET_STEP: usize = 5;
    let locality = match window.require_locality(chain, CheckId::C6, TARGET_STEP) {
        Ok(locality) => locality,
        Err(check) => return check,
    };
    let tag = step_tag(TARGET_STEP);
    let retired_returns = locality
        .scope
        .as_ref()
        .is_some_and(|scope| scope.retiring.contains("returns") && scope.whole_reason.is_none());
    let linger_count = locality.lingering_removals.get("returns").map_or(0, Vec::len);
    let retiring_list =
        locality.scope.as_ref().map_or_else(String::new, |scope| comma_joined(&scope.retiring));
    let reading = format!(
        "`{tag}` retiring `[{retiring_list}]`, `crates/returns` lingering {linger_count}, \
         retiring slice turns {}",
        locality.retiring_turns,
    );
    let evidence = format!("`{tag}.locality.md`, `{tag}.build.stderr`");
    let clean = retired_returns
        && locality.lingering_removals.contains_key("returns")
        && locality.removed_clean()
        && locality.retiring_turns == 0;
    if clean {
        Check::pass(CheckId::C6, reading, evidence)
    } else {
        Check::fail(CheckId::C6, reading, evidence)
    }
}

fn round_trip_summary(delta: &RoundTripDelta) -> String {
    let label = if delta.against_step == 4 {
        "`step-04` (6-route spec)".to_owned()
    } else {
        format!("`{}`", step_tag(delta.against_step))
    };
    format!(
        "LOC {} vs {label} {} ({:+}%, within ±10%: {}, items {} vs {}, tests {} vs {})",
        delta.current.loc,
        delta.base.loc,
        delta.loc_percent_delta(),
        if delta.loc_within_ten_percent() { "yes" } else { "no" },
        delta.current.items,
        delta.base.items,
        delta.current.tests,
        delta.base.tests,
    )
}

fn write_measures(f: &mut Formatter<'_>, chain: &ChainRun, window: StepWindow) -> fmt::Result {
    writeln!(f, "\n#### Measures\n")?;
    write_correctness_bullet(f, chain)?;
    write_locality_bullet(f, chain, window)?;
    write_scope_bullet(f, chain, window)?;
    write_round_trip_bullet(f, chain)?;
    write_cost_bullet(f, chain)?;
    write_amend_bullet(f, chain, window)
}

fn write_correctness_bullet(f: &mut Formatter<'_>, chain: &ChainRun) -> fmt::Result {
    let mut entries = Vec::new();
    for step in chain.active_steps() {
        if let Some(oracle) = &step.oracle {
            if step.is_baseline() {
                entries
                    .push(format!("`{}` {}/{}", step.tag, oracle.step_passed, oracle.step_total));
            } else {
                entries.push(format!(
                    "`{}` carried {}/{} · step {}/{}",
                    step.tag,
                    oracle.carried_passed,
                    oracle.carried_total,
                    oracle.step_passed,
                    oracle.step_total,
                ));
            }
        } else {
            entries.push(format!("`{}` oracle not run", step.tag));
        }
    }
    writeln!(
        f,
        "- **Correctness:** {}",
        if entries.is_empty() { "none".to_owned() } else { entries.join("; ") }
    )
}

fn write_locality_bullet(
    f: &mut Formatter<'_>, chain: &ChainRun, window: StepWindow,
) -> fmt::Result {
    let expected = window.amend_count();
    let carried_ok = chain
        .amend_steps()
        .filter(|step| step.locality.as_ref().is_some_and(StepLocality::carried_byte_identical))
        .count();
    let mut admitted = Vec::new();
    let mut violations = Vec::new();
    for step in chain.amend_steps() {
        if let Some(locality) = &step.locality {
            for item in admitted_touches(locality) {
                admitted.push(format!("`{}` {item}", step.tag));
            }
            for violation in locality.touch_violations() {
                violations.push(format!("`{}` {violation}", step.tag));
            }
        }
    }
    let mut git_calls = Vec::new();
    for step in chain.active_steps() {
        if let Some(locality) = &step.locality {
            for call in &locality.git_calls {
                git_calls.push(format!("`{}` `{}` (`{}`)", step.tag, call.label, call.command));
            }
        }
    }
    let admitted_text = if admitted.is_empty() {
        "admitted touch edits: none".to_owned()
    } else {
        format!("admitted touch edits ({}): {}", admitted.len(), admitted.join(", "))
    };
    let violations_text = if violations.is_empty() {
        "0 touch violations".to_owned()
    } else {
        format!("{} touch violation(s): {}", violations.len(), violations.join("; "))
    };
    let git_text = if git_calls.is_empty() {
        "0 worker `git` shell calls".to_owned()
    } else {
        format!("{} worker `git` shell call(s): {}", git_calls.len(), git_calls.join("; "))
    };
    writeln!(
        f,
        "- **Locality:** {carried_ok}/{expected} steps carried byte-identical; {admitted_text}; \
         {violations_text}; {git_text}."
    )
}

fn write_scope_bullet(f: &mut Formatter<'_>, chain: &ChainRun, window: StepWindow) -> fmt::Result {
    let (ok, expected, detail) = grade_summary(chain, window, |locality| {
        let scope = locality.scope.as_ref()?;
        Some((scope.passed(), &scope.mismatches))
    });
    writeln!(f, "- **Scope:** {ok}/{expected} steps match `step.toml` expected scope ({detail}).")
}

fn write_amend_bullet(f: &mut Formatter<'_>, chain: &ChainRun, window: StepWindow) -> fmt::Result {
    let (ok, expected, detail) = grade_summary(chain, window, |locality| {
        let amend = locality.amend.as_ref()?;
        Some((amend.passed(), &amend.mismatches))
    });
    writeln!(
        f,
        "- **Amend fidelity:** {ok}/{expected} steps match `step.toml` change classes and stems ({detail})."
    )
}

fn grade_summary<'a>(
    chain: &'a ChainRun, window: StepWindow,
    select: impl Fn(&'a StepLocality) -> Option<(bool, &'a [String])>,
) -> (usize, usize, String) {
    let expected = window.amend_count();
    let mut passed = 0;
    let mut mismatches = Vec::new();
    for step in chain.amend_steps() {
        if let Some((ok, step_mismatches)) = step.locality.as_ref().and_then(&select) {
            passed += usize::from(ok);
            for mismatch in step_mismatches {
                mismatches.push(format!("`{}` {mismatch}", step.tag));
            }
        }
    }
    let detail = if mismatches.is_empty() {
        "0 mismatches".to_owned()
    } else {
        format!("mismatches: {}", mismatches.join("; "))
    };
    (passed, expected, detail)
}

fn write_round_trip_bullet(f: &mut Formatter<'_>, chain: &ChainRun) -> fmt::Result {
    let trajectory: Vec<String> = chain
        .active_steps()
        .filter_map(|step| {
            let size = step.locality.as_ref()?.sizes.get("orders")?;
            Some(format!(
                "`{}` {} LOC ({} items, {} tests)",
                step.tag, size.loc, size.items, size.tests
            ))
        })
        .collect();
    let deltas: Vec<String> = chain
        .steps
        .iter()
        .rev()
        .find(|step| {
            !step.resumed && step.locality.as_ref().is_some_and(|l| !l.round_trips.is_empty())
        })
        .and_then(|step| step.locality.as_ref())
        .map_or_default(|locality| locality.round_trips.iter().map(round_trip_summary).collect());
    let path = if trajectory.is_empty() {
        "no `crates/orders` snapshots recorded".to_owned()
    } else {
        trajectory.join(" → ")
    };
    if deltas.is_empty() {
        writeln!(f, "- **Round trip:** `crates/orders` {path}.")
    } else {
        writeln!(f, "- **Round trip:** `crates/orders` {path}; {}.", deltas.join("; "))
    }
}

fn write_cost_bullet(f: &mut Formatter<'_>, chain: &ChainRun) -> fmt::Result {
    let totals: StepCost = chain.active_steps().map(StepCost::from).sum();
    writeln!(
        f,
        "- **Cost:** {} completions ({} specify/amend, {} slice build, {} verify), {} corrected, \
         {} verify repair(s), {} tool calls ({} `edit_files`, {} `write_files`, {} refused, {} shell), \
         {} input / {} cached / {} output ({} reasoning) tokens, {}s wall ({}s specify/amend + {}s build); \
         recorded, not gated.",
        totals.turns(),
        totals.stage_turns,
        totals.slice_turns,
        totals.verify_turns,
        totals.corrected,
        totals.repairs,
        totals.tool_calls(),
        totals.edits,
        totals.writes,
        totals.refused,
        totals.shells,
        totals.tokens.input,
        totals.tokens.cached,
        totals.tokens.output,
        totals.tokens.reasoning,
        totals.wall.as_secs(),
        totals.stage_wall.as_secs(),
        totals.build_wall().as_secs(),
    )
}

fn write_cost_table(f: &mut Formatter<'_>, chain: &ChainRun) -> fmt::Result {
    let mut active = chain.active_steps().peekable();
    if active.peek().is_none() {
        return Ok(());
    }
    writeln!(
        f,
        "\n#### Per-step cost\n\n\
         | step | specify / amend | slice turns | verify turns | corrected | verify repairs | tool calls (edit / write / refused / shell) | input | cached | output | reasoning | wall |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    )?;
    for step in active {
        let cost = StepCost::from(step);
        let phase = if step.is_baseline() { "specify" } else { "amend" };
        writeln!(
            f,
            "| `{}` | {} {phase} ({}s) | {} | {} | {} | {} | {} ({}/{}/{}/{}) | {} | {} | {} | {} | {}s |",
            step.tag,
            cost.stage_turns,
            cost.stage_wall.as_secs(),
            cost.slice_turns,
            cost.verify_turns,
            cost.corrected,
            cost.repairs,
            cost.tool_calls(),
            cost.edits,
            cost.writes,
            cost.refused,
            cost.shells,
            cost.tokens.input,
            cost.tokens.cached,
            cost.tokens.output,
            cost.tokens.reasoning,
            cost.wall.as_secs(),
        )?;
    }
    Ok(())
}

fn admitted_touches(locality: &StepLocality) -> impl Iterator<Item = &str> {
    locality.crates.iter().flat_map(|krate| krate.touched_items.iter().map(String::as_str))
}

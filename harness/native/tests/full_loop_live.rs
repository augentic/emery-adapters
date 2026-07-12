//! The native live loop: the canonical `guest-execute-loop` scenario's
//! `native-live` profile driven once, end to end, against the real
//! cursor-agent backend — `plan author` → the operator stamp → `plan
//! execute` — then graded with the scenario's hard assertions
//! (generic probes plus the registered guest evaluators).
//!
//! `#[ignore]`: requires an authenticated cursor-agent on `PATH` and
//! makes real model calls. The native-live runner (`specify-dev
//! quality`) owns repeated trials and semantic rubric grading; this
//! test proves one trial of the driver.

use std::{env, fs};

use scenario::grade::Evaluators;
use scenario::{AssertionId, Grading, ModelBackend, Outcome, Runtime, catalog, evaluate, grade};
use specify_dev::{guest_loop, verify};

fn cursor_agent_on_path() -> bool {
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|directory| directory.join("cursor-agent").is_file())
    })
}

#[tokio::test]
#[ignore = "live: needs authenticated cursor-agent; run with -- --ignored"]
async fn native_live_trial_passes() {
    let scenario = catalog::load(guest_loop::SCENARIO).expect("canonical scenario");
    let profile = scenario
        .profiles
        .iter()
        .find(|profile| profile.id == "native-live")
        .expect("native-live profile");
    assert_eq!(profile.runtime, Runtime::Native);
    assert_eq!(profile.model, ModelBackend::Live);
    assert_eq!(profile.grading, Grading::Semantic);

    assert!(
        cursor_agent_on_path(),
        "cursor-agent not found on PATH; install it, then `cursor-agent login` or export \
         CURSOR_API_KEY"
    );
    assert!(
        env::var("SPECIFY_DEV_MODEL").as_deref() != Ok("replay"),
        "SPECIFY_DEV_MODEL=replay contradicts the live profile"
    );

    // Persisted (not a tempdir) so a failing live run leaves its
    // evidence behind for inspection.
    let sandbox = env::temp_dir().join(format!("guest-execute-loop-live-{}", std::process::id()));
    if sandbox.exists() {
        fs::remove_dir_all(&sandbox).expect("clearing a stale sandbox");
    }
    eprintln!("sandbox: {}", sandbox.display());

    let steps = guest_loop::drive(&sandbox).await.expect("the driver completes setup");
    for (id, step) in &steps {
        assert_eq!(step.exit_code, 0, "step `{id}` failed:\n{}\n{}", step.stdout, step.stderr);
    }

    let execution = grade::Execution::new(&sandbox, steps);
    let evaluators = Evaluators::default()
        .with(AssertionId::GuestJournalCadence, evaluate::guest::journal_cadence)
        .with(AssertionId::GuestGeneratedCrateVerifies, verify::generated_crates_verify);
    let results = grade::hard_with(&scenario, &execution, &evaluators);
    for result in &results {
        assert_eq!(
            result.outcome,
            Outcome::Pass,
            "hard assertion `{}` failed: {:?}",
            result.id,
            result.detail
        );
    }
}

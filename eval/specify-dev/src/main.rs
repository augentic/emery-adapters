//! `specify-dev` — native dev shim and eval harness: the shared
//! harness entrypoints over the first-party catalog. Three modes:
//! CLI (default), HTTP (`serve`), and live-model eval (`eval`).

mod grade;

use std::process::ExitCode;

use anyhow::Result;
use harness::inputs::TrialInputs;
use harness::scenario::Scenarios;
use harness::trial::{self, Profile};
use harness::{command, http};
use specify_dev::catalog::FirstParty;
use specify_dev::paths;

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    match argv.get(1).map(String::as_str) {
        Some("serve") => report(http::serve::<FirstParty>(&argv[1..]).await),
        Some("eval") => report(eval(&argv[1..]).await),
        _ => ExitCode::from(command::run::<FirstParty>(argv).await),
    }
}

async fn eval(argv: &[String]) -> Result<ExitCode> {
    trial::run::<FirstParty>(&profile()?, argv).await
}

fn report(outcome: Result<ExitCode>) -> ExitCode {
    outcome.unwrap_or_else(|err| {
        eprintln!("specify-dev: {err:#}");
        ExitCode::FAILURE
    })
}

// The adapters-repo trial: the contracts-bound change over the shared
// `examples/change/trial.env` inputs and seed tree, graded by the
// contracts baseline checks.
fn profile() -> Result<Profile> {
    let inputs = TrialInputs::load(&paths::trial_env())?;
    Ok(Profile {
        sandbox: paths::eval_sandbox(),
        seed: Some(paths::seed_dir()),
        init: argv(&["init", "contracts", "--name", &inputs.project_name]),
        author: argv(&[
            "plan",
            "author",
            &inputs.change,
            "--intent",
            &inputs.intent,
            "--source",
            &inputs.source,
        ]),
        change: inputs.change,
        authored: None,
        grade: grade::run,
        scenarios: Some(Scenarios {
            dir: paths::scenarios_dir(),
            sandbox: paths::scenarios_sandbox(),
        }),
    })
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(ToString::to_string).collect()
}

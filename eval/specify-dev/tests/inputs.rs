//! Trial input parity between the Rust parser and shell `source`
//! over the checked-in `examples/change/trial.env` definition.
//! Generic parser refusals live in the shared harness's own suite.

use std::process::Command;

use harness::inputs::TrialInputs;
use specify_dev::paths;

#[test]
fn checked_in_definition() {
    let inputs = TrialInputs::load(&paths::trial_env()).expect("trial.env parses");

    let (key, binding) = inputs.source.split_once('=').expect("source is `key=adapter:path`");
    let (adapter, path) = binding.split_once(':').expect("binding is `adapter:path`");
    assert!(!key.is_empty() && adapter == "documentation", "source binds the docs adapter");

    let seed = paths::seed_dir().join(path);
    let populated = seed
        .read_dir()
        .unwrap_or_else(|err| panic!("shared seed {} unreadable: {err}", seed.display()))
        .next()
        .is_some();
    assert!(populated, "shared seed {} holds the surveyed docs", seed.display());
}

#[test]
fn shell_parity() {
    let inputs = TrialInputs::load(&paths::trial_env()).expect("trial.env parses");
    let script = format!(
        ". '{}' && printf '%s\\n' \"$TRIAL_PROJECT_NAME\" \"$TRIAL_CHANGE\" \
         \"$TRIAL_SOURCE\" \"$TRIAL_INTENT\"",
        paths::trial_env().display()
    );
    let output = Command::new("sh").args(["-eu", "-c", &script]).output().expect("sh runs");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    let shell: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        shell,
        [
            inputs.project_name.as_str(),
            inputs.change.as_str(),
            inputs.source.as_str(),
            inputs.intent.as_str()
        ],
        "shell `source` and the Rust parser must agree"
    );
}

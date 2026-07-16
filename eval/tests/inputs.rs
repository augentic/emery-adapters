//! Model-free parity coverage of the shared change-trial inputs: the
//! checked-in `examples/change/trial.env` means the same thing to the
//! native trial's Rust parser and to the wasm task's shell `source`,
//! and both rungs' seed is the one shared tree.

use std::process::Command;

use eval::inputs::{self, TrialInputs};

/// The checked-in definition parses, and the `source` binding points
/// at a populated directory inside the shared seed tree.
#[test]
fn checked_in_definition() {
    let inputs = TrialInputs::load().expect("trial.env parses");

    let (key, binding) = inputs.source.split_once('=').expect("source is `key=adapter:path`");
    let (adapter, path) = binding.split_once(':').expect("binding is `adapter:path`");
    assert!(!key.is_empty() && adapter == "documentation", "source binds the docs adapter");

    let seed = inputs::seed_dir().join(path);
    let populated = seed
        .read_dir()
        .unwrap_or_else(|err| panic!("shared seed {} unreadable: {err}", seed.display()))
        .next()
        .is_some();
    assert!(populated, "shared seed {} holds the surveyed docs", seed.display());
}

/// Shell parity: `source`-ing the file yields exactly the values the
/// Rust parser reads, so the two rungs cannot drift.
#[test]
fn shell_parity() {
    let inputs = TrialInputs::load().expect("trial.env parses");
    let script = format!(
        ". '{}' && printf '%s\\n' \"$TRIAL_PROJECT_NAME\" \"$TRIAL_CHANGE\" \
         \"$TRIAL_SOURCE\" \"$TRIAL_INTENT\"",
        inputs::path().display()
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

/// The parser refuses anything outside the documented sourceable
/// subset, so a value the shell would reinterpret never parses.
mod refusals {
    use super::*;

    const VALID: &str = "TRIAL_PROJECT_NAME=\"orders\"\nTRIAL_CHANGE=\"orders\"\n\
                         TRIAL_SOURCE=\"docs=documentation:docs\"\nTRIAL_INTENT=\"do it\"\n";

    #[test]
    fn valid_parses() {
        TrialInputs::parse(VALID).expect("documented shape parses");
    }

    #[test]
    fn unquoted_value() {
        let err = TrialInputs::parse(&VALID.replace("\"orders\"\nTRIAL_CHANGE", "orders\nTRIAL_CHANGE"))
            .expect_err("unquoted values refuse");
        assert!(format!("{err:#}").contains("double-quoted"), "{err:#}");
    }

    #[test]
    fn expansion_refused() {
        let err = TrialInputs::parse(&VALID.replace("do it", "do $HOME"))
            .expect_err("shell expansion characters refuse");
        assert!(format!("{err:#}").contains("shell would expand"), "{err:#}");
    }

    #[test]
    fn missing_key() {
        let body = VALID.replace("TRIAL_INTENT=\"do it\"\n", "");
        let err = TrialInputs::parse(&body).expect_err("a missing key refuses");
        assert!(format!("{err:#}").contains("TRIAL_INTENT"), "{err:#}");
    }

    #[test]
    fn unknown_key() {
        let body = format!("{VALID}TRIAL_SURPRISE=\"x\"\n");
        let err = TrialInputs::parse(&body).expect_err("an unknown key refuses");
        assert!(format!("{err:#}").contains("TRIAL_SURPRISE"), "{err:#}");
    }

    #[test]
    fn duplicate_key() {
        let body = format!("{VALID}TRIAL_CHANGE=\"again\"\n");
        let err = TrialInputs::parse(&body).expect_err("a duplicate key refuses");
        assert!(format!("{err:#}").contains("duplicate"), "{err:#}");
    }
}

//! Unit tests for `vectis scaffold` paths that reach module-private items: the
//! `.gitignore` merge (`runtime::merge_gitignore`, `pub(super)`) and the `run`
//! dispatcher (which mutates the `PROJECT_DIR` env var). Plan-shape,
//! substitution, capability-gating, and overwrite-refusal coverage is re-homed
//! to the crate-level `tests/engine/scaffold.rs` integration suite.

use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

use tempfile::tempdir;

use super::*;

fn versions() -> Versions {
    Versions::embedded().expect("embedded versions parse")
}

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn write_plan_merges_existing_gitignore() {
    // `specify init` writes a root `.gitignore` in every project, so
    // the bootstrap path scaffolds into an initialised repo: the
    // template's missing lines append; operator content survives.
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), ".specify/cache/\n/target\n").unwrap();
    let plan = plan_core("Counter", "com.vectis.counter", &[], &versions()).unwrap();
    write_plan(dir.path(), &plan).expect("gitignore collision merges");

    let merged = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(merged.starts_with(".specify/cache/\n/target\n"), "operator content survives");
    assert!(merged.contains("# Vectis scaffold"));
    assert!(merged.contains(".DS_Store"), "template lines appended");
    assert_eq!(merged.matches("/target").count(), 1, "duplicate lines are not re-appended");
    assert!(dir.path().join("shared/src/app.rs").exists(), "rest of the plan writes normally");

    // Idempotent: a second merge pass appends nothing.
    let plan_again = plan_core("Counter", "com.vectis.counter", &[], &versions()).unwrap();
    let gitignore_template = plan_again
        .files
        .iter()
        .find(|file| file.relative_path == ".gitignore")
        .expect("core plan carries .gitignore");
    runtime::merge_gitignore(&dir.path().join(".gitignore"), &gitignore_template.contents)
        .expect("re-merge succeeds");
    let remerged = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert_eq!(merged, remerged, "second merge is a no-op");
}

#[test]
fn run_writes_under_project_dir() {
    let _guard = env_lock();
    let dir = tempdir().unwrap();
    let previous = std::env::var_os("PROJECT_DIR");
    #[expect(unsafe_code, reason = "edition-2024 set_var is unsafe; env_lock serializes access")]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    let () = unsafe { std::env::set_var("PROJECT_DIR", dir.path()) };

    let command = ScaffoldCommand::Core(CoreArgs {
        common: CommonArgs {
            app_name: "Counter".into(),
            caps: None,
            version_file: None,
        },
        android_package: None,
    });
    let value = run(&command).expect("run succeeds");
    assert_eq!(value["target"], "core");
    assert!(dir.path().join("shared/src/app.rs").is_file());

    #[expect(
        unsafe_code,
        reason = "edition-2024 set_var/remove_var are unsafe; env_lock serializes access"
    )]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("PROJECT_DIR", value),
            None => std::env::remove_var("PROJECT_DIR"),
        }
    }
}

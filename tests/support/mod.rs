//! Scenario support shared by the seam suites: the one runner that puts a
//! component under the omnia runtime behind the `source_extract` driver, so
//! `tests/source.rs` and `tests/probe.rs` build the deployment the same way
//! and differ only in the component under test, the mode the driver runs,
//! and what each asserts of the model's record afterwards.

use omnia::ExitStatus;
use omnia_test::host::{Backends, Deployment, Scratch, ScriptedModel};
use omnia_wasi_model::WasiModel;

/// Runs the driver in the mode `args` names against `adapter` as the
/// component under test, `project` mounted read-only as `.` and `model`
/// answering the host side; requires a clean exit — the driver traps on any
/// check that fails — and the script exactly consumed, and returns the
/// model's record.
pub async fn run(
    adapter: &str, project: &Scratch, args: &[&str], model: ScriptedModel,
) -> ScriptedModel {
    let backends = Backends::defaults().await.model(model.clone());
    let status = Deployment::new()
        .link(["emery:adapter/source@0.1.0"])
        .guest("caller", test_programs::SOURCE_EXTRACT)
        .guest(test_programs::ADAPTER, adapter)
        .command("caller")
        .mount(project.mount(false))
        .args(args.iter().copied())
        .run_host::<WasiModel, _>(backends)
        .await
        .expect("the caller runs");
    assert_eq!(status, ExitStatus::SUCCESS, "the driver's checks failed against `{adapter}`");
    model.assert_exhausted();
    model
}

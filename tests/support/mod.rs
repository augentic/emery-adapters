//! Scenario support
//!
//! The one runner the seam suites share: a component under the omnia
//! runtime behind the `source_extract` driver, over a scripted host model.

use omnia::ExitStatus;
use omnia_test::host::{Backends, Deployment, Scratch, ScriptedModel};
use omnia_wasi_model::WasiModel;

/// Runs the driver in the mode `args` names against `adapter`, `project`
/// mounted read-only as `.`; requires a clean exit and the script exactly
/// consumed, and returns the model's record.
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

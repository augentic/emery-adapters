//! The seam's own failure lowering, proved with the fixture adapters under
//! `crates/test-programs/programs/probe/` in place of a shipped one: each
//! probe fails `extract` with one WIT `error` arm, and the `source_refused`
//! driver sees it lifted back to the Omnia error class on the caller's side
//! of `emery:adapter/source`, with no model turn consumed. Every scenario
//! runs the real components through the omnia runtime.

#![cfg(not(target_arch = "wasm32"))]

use omnia::ExitStatus;
use omnia_test::host::{Backends, Deployment, ScriptedModel, scratch};
use omnia_wasi_model::WasiModel;

// Every probe program in `crates/test-programs` must have a matching test
// here; a new probe without one fails to compile.
test_programs::foreach_probe!();

/// Runs the `source_refused` driver against `probe` as the adapter under
/// test, requiring the refusal to cross the seam as `code`; the driver traps
/// on any other class, and nothing is scripted because a probe never reaches
/// the model.
async fn refused_by(probe: &str, code: &str) {
    let project = scratch();
    let model = ScriptedModel::default();
    let backends = Backends::defaults().await.model(model.clone());
    let status = Deployment::new()
        .link(["emery:adapter/source@0.1.0"])
        .guest("caller", test_programs::SOURCE_REFUSED)
        .guest(test_programs::ADAPTER, probe)
        .command("caller")
        .mount(project.mount(false))
        .args([code])
        .run_host::<WasiModel, _>(backends)
        .await
        .expect("the caller runs");
    assert_eq!(status, ExitStatus::SUCCESS, "`{probe}` did not refuse as `{code}`");
    assert!(model.seen().is_empty(), "a probe never reaches the model");
}

// `bad_request!` lowers onto the WIT `invalid-request` arm and lifts back as
// `bad_request`: the class the engine reads as an adapter refusing its input.
#[tokio::test]
async fn probe_refusing() {
    refused_by(test_programs::PROBE_REFUSING, "bad_request").await;
}

// `bad_gateway!` lowers onto the `internal` arm — every class but a refusal
// shares it — and lifts back as `bad_gateway`: an adapter's own failure.
#[tokio::test]
async fn probe_upstream() {
    refused_by(test_programs::PROBE_UPSTREAM, "bad_gateway").await;
}

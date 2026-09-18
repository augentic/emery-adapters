//! Provides the runtime harness shared by component suites.
//!
//! The harness runs a component behind the `source_extract` driver with a
//! scripted host model. [`Barrier`] can hold completions until a required
//! number of requests are pending together.

// Compiled into every component suite; each uses a subset.
#![allow(dead_code, reason = "shared by suites that each use a subset")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use omnia::ExitStatus;
use omnia_test::host::{Backends, Deployment, Scratch, ScriptedModel};
use omnia_wasi_model::{
    Answer, Error, FutureResult, Limits, Request, ToolHost, WasiModel, WasiModelCtx,
};

/// How long a held completion waits for the rest of its party.
///
/// Generous on a loaded CI box; paid only when the host serialises what the
/// guest issued together.
const HOLD: Duration = Duration::from_secs(5);

/// A host model the runner holds to its script.
///
/// Every scripted turn is consumed, and none is requested past it.
pub trait Strict: WasiModelCtx + Clone {
    /// Returns the script and its consumption record.
    fn script(&self) -> &ScriptedModel;
}

impl Strict for ScriptedModel {
    fn script(&self) -> &ScriptedModel {
        self
    }
}

/// A [`ScriptedModel`] that answers a completion only once `parties` are pending.
///
/// Concurrent requests pass the barrier together. A serial host reaches the
/// bounded [`HOLD`] timeout instead, identifying the missing party, and
/// every completion after that fails at once: a party the guest fills late —
/// the SDK puts a seam whose turn failed upstream once more — proves nothing
/// about what the host ran together. The barrier resets for each party.
#[derive(Clone, Debug)]
pub struct Barrier {
    inner: ScriptedModel,
    gate: Arc<tokio::sync::Barrier>,
    parties: usize,
    expired: Arc<AtomicBool>,
}

impl Barrier {
    /// Holds each of `model`'s completions until `parties` are pending.
    #[must_use]
    pub fn new(model: ScriptedModel, parties: usize) -> Self {
        Self {
            inner: model,
            gate: Arc::new(tokio::sync::Barrier::new(parties)),
            parties,
            expired: Arc::default(),
        }
    }

    // The diagnostic a completion fails with once a hold has expired.
    fn serialised(&self) -> Error {
        Error::Backend(format!(
            "completion held {HOLD:?} without {} pending at once: the host serialises one \
             guest's completions",
            self.parties
        ))
    }
}

impl Strict for Barrier {
    fn script(&self) -> &ScriptedModel {
        &self.inner
    }
}

impl WasiModelCtx for Barrier {
    fn complete(&self, request: Request, tool_host: Arc<dyn ToolHost>) -> FutureResult<Answer> {
        let this = self.clone();
        Box::pin(async move {
            if this.expired.load(Ordering::SeqCst) {
                return Err(this.serialised().into());
            }
            if tokio::time::timeout(HOLD, this.gate.wait()).await.is_err() {
                this.expired.store(true, Ordering::SeqCst);
                return Err(this.serialised().into());
            }
            this.inner.complete(request, tool_host).await
        })
    }

    // Qualified: `ScriptedModel`'s inherent `limits(self, Limits)` builder
    // shadows the trait method.
    fn limits(&self) -> Limits {
        WasiModelCtx::limits(&self.inner)
    }
}

/// Runs the driver in the mode `args` names against `adapter`.
///
/// `project` is mounted read-only as `.`. A clean exit and an exactly
/// consumed script are required; the model's record is returned.
///
/// # Panics
///
/// Panics when deployment fails, the driver exits unsuccessfully, or the
/// model script is not consumed exactly.
pub async fn run<M: Strict>(adapter: &str, project: &Scratch, args: &[&str], model: M) -> M {
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
    model.script().assert_exhausted();
    model
}

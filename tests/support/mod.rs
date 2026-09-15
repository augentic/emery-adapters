//! Provides the one runner the seam suites share.
//!
//! The runner puts a component under the omnia runtime behind the
//! `source_extract` driver, over a scripted host model — or a [`Barrier`]
//! around one, which holds each completion until the rest of its party is
//! pending too.

// Compiled into every seam suite; each uses a subset.
#![allow(dead_code, reason = "shared by suites that each use a subset")]

use std::sync::Arc;
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
    /// The script and its record.
    fn script(&self) -> &ScriptedModel;
}

impl Strict for ScriptedModel {
    fn script(&self) -> &ScriptedModel {
        self
    }
}

/// A [`ScriptedModel`] that answers a completion only once `parties` are pending.
///
/// This makes the property the SDK's fan-out rests on observable. A guest
/// that issues its completions together sees them all answered; a host that
/// serialised them would hold the first alone until [`HOLD`] elapsed and fail
/// it, naming the party that never arrived. The gate is reusable, so one
/// barrier serves every `extract` a run makes.
#[derive(Clone, Debug)]
pub struct Barrier {
    inner: ScriptedModel,
    gate: Arc<tokio::sync::Barrier>,
    parties: usize,
}

impl Barrier {
    /// Holds each of `model`'s completions until `parties` are pending.
    #[must_use]
    pub fn new(model: ScriptedModel, parties: usize) -> Self {
        Self {
            inner: model,
            gate: Arc::new(tokio::sync::Barrier::new(parties)),
            parties,
        }
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
            if tokio::time::timeout(HOLD, this.gate.wait()).await.is_err() {
                return Err(Error::Backend(format!(
                    "completion held {HOLD:?} without {} pending at once: the host serialises one \
                     guest's completions",
                    this.parties
                ))
                .into());
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

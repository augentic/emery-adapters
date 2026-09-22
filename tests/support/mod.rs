//! Provides the runtime harness shared by component suites.
//!
//! The harness runs a component behind the `source_extract` driver with a
//! scripted host model. [`Barrier`] can hold completions until a required
//! number of requests are pending together.

// Compiled into every component suite; each uses a subset.
#![allow(dead_code, reason = "shared by suites that each use a subset")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use omnia::{ExitStatus, FutureResult, HostCtx, Provides, Telemetry};
use omnia_test::host::{Backends, Deployment, Scratch, ScriptedModel};
use omnia_wasi_model::{Answer, Error, Limits, Request, ToolHost, WasiModel, WasiModelCtx};
use omnia_wasi_otel::{WasiOtel, WasiOtelCtx};
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use tracing::Instrument as _;

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

/// Guest spans exported during one traced adapter run.
#[derive(Clone, Debug, Default)]
pub struct Recording {
    traces: Arc<Mutex<Vec<ExportTraceServiceRequest>>>,
}

impl Recording {
    /// Returns every exported span name in arrival order.
    #[must_use]
    pub fn span_names(&self) -> Vec<String> {
        self.traces
            .lock()
            .expect("traces lock")
            .iter()
            .flat_map(|request| &request.resource_spans)
            .flat_map(|resource| &resource.scope_spans)
            .flat_map(|scope| &scope.spans)
            .map(|span| span.name.clone())
            .collect()
    }
}

impl WasiOtelCtx for Recording {
    fn export_traces(&self, request: ExportTraceServiceRequest) -> FutureResult<()> {
        self.traces.lock().expect("traces lock").push(request);
        Box::pin(async { Ok(()) })
    }

    fn export_metrics(&self, _request: ExportMetricsServiceRequest) -> FutureResult<()> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone, Debug)]
struct Traced<M> {
    model: M,
    otel: Recording,
}

impl<M: WasiModelCtx + Clone> Provides<WasiModel> for Traced<M> {
    fn borrow(&mut self) -> <WasiModel as HostCtx>::Borrow<'_> {
        &mut self.model
    }
}

impl<M: WasiModelCtx + Clone> Provides<WasiOtel> for Traced<M> {
    fn borrow(&mut self) -> <WasiOtel as HostCtx>::Borrow<'_> {
        &mut self.otel
    }
}

// The guest environment default mirrors the shipped `emery` runtime's, so an adapter's tracing
// opens here as it does under the engine.
fn deployment(adapter: &str, project: &Scratch, args: &[&str]) -> Deployment {
    Deployment::new()
        .link(["emery:adapter/source@0.1.0"])
        .guest("caller", test_programs::SOURCE_EXTRACT)
        .guest(test_programs::ADAPTER, adapter)
        .command("caller")
        .mount(project.mount(false))
        .env([("RUST_LOG", "emery_sdk=info")])
        .args(args.iter().copied())
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
    let status = deployment(adapter, project, args)
        .run_host::<WasiModel, _>(backends)
        .await
        .expect("the caller runs");
    assert_eq!(status, ExitStatus::SUCCESS, "the driver's checks failed against `{adapter}`");
    model.script().assert_exhausted();
    model
}

/// Runs an adapter under recording telemetry and returns its model and spans.
///
/// Host telemetry is installed here, once per test process: the host grafts
/// a guest's spans onto its own live span and drops them without one, so the
/// run is driven inside a host span. `cargo nextest` gives every test its own
/// process; a second call in one process fails at the install.
///
/// # Panics
///
/// Panics when telemetry installation or deployment fails, the driver exits
/// unsuccessfully, or the model script is not consumed exactly.
pub async fn traced<M: Strict>(
    adapter: &str, project: &Scratch, args: &[&str], model: M,
) -> (M, Recording) {
    Telemetry::new("adapter-tests").filter("info").build().expect("host telemetry installs");

    let otel = Recording::default();
    let backends = Traced {
        model: model.clone(),
        otel: otel.clone(),
    };
    let status = deployment(adapter, project, args)
        .run(backends, |deployment| {
            deployment.host::<WasiModel, Traced<M>>()?;
            deployment.host::<WasiOtel, Traced<M>>()?;
            Ok(())
        })
        .instrument(tracing::info_span!("adapter_test"))
        .await
        .expect("the caller runs");
    assert_eq!(status, ExitStatus::SUCCESS, "the driver's checks failed against `{adapter}`");
    model.script().assert_exhausted();
    (model, otel)
}

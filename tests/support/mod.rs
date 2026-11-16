//! Shared runtime test harness for component and probe suites.

#![expect(dead_code, reason = "shared by suites that each use a subset")]

use std::borrow::Borrow;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use omnia::{ExitStatus, FutureResult, Telemetry};
use omnia_test::host::{Backends, Deployment, Scratch, ScriptedModel};
use omnia_wasi_model::{Answer, Error, Limits, ModelClient, Request, ToolHost, WasiModel};
use omnia_wasi_otel::{OtelClient, OtelDefault};
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use tracing::Instrument as _;

// Generous on a loaded CI box; paid only when the host serialises what the
// guest issued together.
const HOLD: Duration = Duration::from_secs(5);

// Holds each completion until `parties` are pending together.
//
// Once `HOLD` expires, every subsequent completion fails immediately: a party
// the guest fills late proves nothing about concurrency because the SDK retries
// a seam whose turn failed upstream.
#[derive(Clone, Debug)]
pub struct Barrier {
    model: ScriptedModel,
    gate: Arc<tokio::sync::Barrier>,
    parties: usize,
    expired: Arc<AtomicBool>,
}

impl Barrier {
    #[must_use]
    pub fn new(model: ScriptedModel, parties: usize) -> Self {
        Self {
            model,
            gate: Arc::new(tokio::sync::Barrier::new(parties)),
            parties,
            expired: Arc::default(),
        }
    }

    fn serialised(&self) -> Error {
        Error::Backend(format!(
            "completion held {HOLD:?} without {} pending at once: the host serialises one \
             guest's completions",
            self.parties
        ))
    }
}

impl Deref for Barrier {
    type Target = ScriptedModel;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl Borrow<ScriptedModel> for Barrier {
    fn borrow(&self) -> &ScriptedModel {
        &self.model
    }
}

impl ModelClient for Barrier {
    fn complete(&self, request: Request, tool_host: Arc<dyn ToolHost>) -> FutureResult<Answer> {
        let this = self.clone();
        Box::pin(async move {
            if this.expired.load(Ordering::Relaxed) {
                return Err(this.serialised().into());
            }
            if tokio::time::timeout(HOLD, this.gate.wait()).await.is_err() {
                this.expired.store(true, Ordering::Relaxed);
                return Err(this.serialised().into());
            }
            this.model.complete(request, tool_host).await
        })
    }

    fn limits(&self) -> Limits {
        // HACK: the inherent `limits(self, Limits)` builder shadows the trait method
        ModelClient::limits(&self.model)
    }
}

pub trait Strict: ModelClient + Borrow<ScriptedModel> + Clone {}

impl<M: ModelClient + Borrow<ScriptedModel> + Clone> Strict for M {}

// Guest spans exported during one traced adapter run.
#[derive(Clone, Debug, Default)]
pub struct Recording {
    traces: Arc<Mutex<Vec<ExportTraceServiceRequest>>>,
}

impl Recording {
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

impl OtelClient for Recording {
    fn export_traces(&self, request: ExportTraceServiceRequest) -> FutureResult<()> {
        self.traces.lock().expect("traces lock").push(request);
        Box::pin(async { Ok(()) })
    }

    fn export_metrics(&self, _request: ExportMetricsServiceRequest) -> FutureResult<()> {
        Box::pin(async { Ok(()) })
    }
}

pub async fn run<M: Strict>(adapter: &str, project: &Scratch, args: &[&str], model: M) -> M {
    Driver::Source.execute(adapter, project, args, model, OtelDefault).await
}

pub async fn build<M: Strict>(adapter: &str, project: &Scratch, args: &[&str], model: M) -> M {
    Driver::Target.execute(adapter, project, args, model, OtelDefault).await
}

// Host telemetry is installed once per test process, and the run is driven
// inside a host span because the host drops guest spans when none is active.
pub async fn traced<M: Strict>(
    adapter: &str, project: &Scratch, args: &[&str], model: M,
) -> (M, Recording) {
    Telemetry::new("adapter-tests").filter("info").build().expect("host telemetry installs");

    let otel = Recording::default();
    let model = Driver::Source
        .execute(adapter, project, args, model, otel.clone())
        .instrument(tracing::info_span!("adapter_test"))
        .await;
    (model, otel)
}

const CALLER: &str = "caller";

#[derive(Clone, Copy)]
enum Driver {
    Source,
    Target,
}

impl Driver {
    // No level is named: command mode's `info` default is the shipped runtime's on a bare run.
    fn deployment(self, adapter: &str, project: &Scratch, args: &[&str]) -> Deployment {
        let (program, writable) = match self {
            Self::Source => (test_programs::SOURCE_EXTRACT, false),
            Self::Target => (test_programs::BUILDER_BUILD, true),
        };
        Deployment::new()
            .guest(CALLER, program)
            .guest(test_programs::ADAPTER, adapter)
            .command(CALLER)
            .mount(project.mount(writable))
            .args(args.iter().copied())
    }

    async fn execute<M, O>(
        self, adapter: &str, project: &Scratch, args: &[&str], model: M, otel: O,
    ) -> M
    where
        M: Strict,
        O: OtelClient + Clone,
    {
        let backends = Backends::defaults().await.model(model.clone()).otel(otel);
        let status = self
            .deployment(adapter, project, args)
            .run_host::<WasiModel, _>(backends)
            .await
            .expect("the caller runs");
        assert_eq!(status, ExitStatus::SUCCESS, "the driver's checks failed against `{adapter}`");
        model.borrow().assert_exhausted();
        model
    }
}

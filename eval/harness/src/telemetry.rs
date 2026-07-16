//! Per-leg request telemetry over a [`Model`] backend.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use omnia_guest::model::{Error, Format, Model, Reply, Request};

/// Counts completion requests per judgment leg before delegating.
#[derive(Clone, Debug)]
pub struct Telemetry<M> {
    inner: M,
    counts: Arc<Mutex<BTreeMap<String, usize>>>,
}

impl<M> Telemetry<M> {
    /// Wrap `inner` with an empty tally.
    pub fn new(inner: M) -> Self {
        Self {
            inner,
            counts: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Requests per leg, in leg-name order.
    ///
    /// # Panics
    ///
    /// Panics when the tally lock is poisoned (never in practice).
    #[must_use]
    pub fn counts(&self) -> BTreeMap<String, usize> {
        self.counts.lock().expect("the tally is never poisoned").clone()
    }
}

impl<M: Model> Model for Telemetry<M> {
    async fn create(&self, request: Request) -> Result<Reply, Error> {
        let leg = match &request.format {
            Format::Schema(schema) => schema.name.clone(),
            Format::Json => "json".to_string(),
            Format::Text => "text".to_string(),
        };
        *self.counts.lock().expect("the tally is never poisoned").entry(leg).or_default() += 1;
        self.inner.create(request).await
    }
}

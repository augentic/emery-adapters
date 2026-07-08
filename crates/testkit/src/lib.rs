//! Test doubles for the adapter test suites.
//!
//! Consumed only through `[dev-dependencies]`, so nothing here can reach
//! a shipped guest component's dependency graph — the structural
//! guarantee that keeps test support out of mainline adapter code.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::VecDeque;
use std::sync::Mutex;

use adapter::{Error, Model, Reply, Request};

/// Scripted [`Model`] provider for native tests: replies are served in
/// FIFO order and every request is recorded for assertion.
#[derive(Debug, Default)]
pub struct MockModel {
    replies: Mutex<VecDeque<Result<Reply, Error>>>,
    requests: Mutex<Vec<Request>>,
}

impl MockModel {
    /// A mock that answers each call with the next scripted result.
    #[must_use]
    pub fn scripted(replies: impl IntoIterator<Item = Result<Reply, Error>>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// A mock whose every call succeeds with the given answers, in order.
    #[must_use]
    pub fn answering(answers: impl IntoIterator<Item = &'static str>) -> Self {
        Self::scripted(answers.into_iter().map(|answer| {
            Ok(Reply {
                answer: answer.to_string(),
            })
        }))
    }

    /// Every request the mock has served, in call order.
    ///
    /// # Panics
    ///
    /// Panics when the interior lock is poisoned — only possible after a
    /// prior panic in the same test.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().expect("mock lock").clone()
    }
}

impl Model for MockModel {
    async fn create(&self, request: Request) -> Result<Reply, Error> {
        self.requests.lock().expect("mock lock").push(request);
        self.replies
            .lock()
            .expect("mock lock")
            .pop_front()
            .expect("MockModel exhausted: script more replies")
    }
}

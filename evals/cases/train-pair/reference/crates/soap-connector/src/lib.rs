//! # Train SOAP Connector
//!
//! Processes SOAP requests and forwards to the `position-adapter` topic.

mod handler;

pub use handler::*;

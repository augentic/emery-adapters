//! # Train Position Transformer
//!
//! Transforms train update messages into vehicle location events.

mod handler;
mod stops;
mod train_update;
mod vehicle_event;

use omnia_sdk::Error;
use thiserror::Error;

pub use self::handler::*;
pub use self::stops::StopInfo;
pub use self::train_update::*;
pub use self::vehicle_event::*;

/// Errors raised while validating an inbound train update message.
///
/// This type covers the train update consumed from messaging.
#[derive(Error, Debug)]
pub enum TrainError {
    /// The message timestamp is invalid (too old or future-dated).
    #[error("{0}")]
    BadTime(String),

    /// The message contains no updates or the arrival/departure time is
    /// invalid (negative or 0).
    #[error("{0}")]
    NoUpdate(String),

    /// The XML is invalid.
    #[error("{0}")]
    InvalidXml(String),
}

impl TrainError {
    fn code(&self) -> String {
        match self {
            Self::BadTime(_) => "bad_time".to_string(),
            Self::NoUpdate(_) => "no_update".to_string(),
            Self::InvalidXml(_) => "invalid_message".to_string(),
        }
    }
}

impl From<TrainError> for Error {
    fn from(err: TrainError) -> Self {
        Self::BadRequest {
            code: err.code(),
            description: err.to_string(),
        }
    }
}

impl From<quick_xml::DeError> for TrainError {
    fn from(err: quick_xml::DeError) -> Self {
        Self::InvalidXml(err.to_string())
    }
}

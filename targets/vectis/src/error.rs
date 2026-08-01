//! Unified terminal-error type shared by the vectis engines.

use std::io;

use thiserror::Error;

/// Terminal failure modes for the vectis engines.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VectisError {
    /// Filesystem I/O failure.
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// The project structure or requested input is invalid or unreadable.
    #[error("invalid project: {message}")]
    InvalidProject {
        /// Diagnostic describing what is wrong.
        message: String,
    },

    /// An internal invariant was violated.
    #[error("internal error: {message}")]
    Internal {
        /// Diagnostic describing what went wrong.
        message: String,
    },
}

//! # Train Common
//!
//! Logic common to the train domain: the Block Management API client, the
//! configuration-key catalog, and the canonical route/topic tables.

pub mod block_mgt;
pub mod config;
pub mod routes;

/// The local timezone of the rail network.
///
/// All schedule arithmetic (service dates, seconds-since-midnight offsets)
/// is performed in this zone.
pub const TIMEZONE: chrono_tz::Tz = chrono_tz::Pacific::Auckland;

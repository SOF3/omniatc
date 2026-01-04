use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::Score;

/// Game statistics.
#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Stats {
    /// Current score.
    pub score:               Score,
    /// Total number of objects with runway arrival destination completed.
    ///
    /// Does not include apron arrivals.
    pub num_runway_arrivals: u32,
    /// Total number of objects with apron arrival destination completed.
    pub num_apron_arrivals:  u32,
    /// Total number of departures completed.
    pub num_departures:      u32,
    /// Number of conflicting pairs that have been detected.
    pub num_conflicts:       u32,
    /// Total duration-pair time of all detected conflicts.
    pub total_conflict_time: Duration,
}

use serde::{Deserialize, Serialize};

use crate::Score;

/// Game statistics.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    /// Current score.
    pub score:          Score,
    /// Total number of arrivals completed.
    pub num_arrivals:   u32,
    /// Total number of departures completed.
    pub num_departures: u32,
}

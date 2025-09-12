use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata describing the file.
#[derive(Clone, Serialize, Deserialize)]
pub struct Meta {
    /// An identifier for this file.
    ///
    /// For published maps,
    /// this is conventionally the ICAO code for the airport/airspace.
    ///
    /// This ID is used to identify different versions of the same map or scenario
    /// to allow user updates.
    ///
    /// For local save files, this ID is simply the filename.
    pub id:          String,
    /// Title of the map.
    ///
    /// For published maps, this should be the airport/airspace name.
    ///
    /// For local save files, this is renameable but defaults to
    /// inheriting the title of the map/scenario it was created from.
    pub title:       String,
    /// A human-readable description of the map.
    pub description: String,
    /// Authors of the map. Only for display.
    pub authors:     Vec<String>,
    /// Tags for categorizing and searching maps.
    pub tags:        HashMap<String, String>,
}

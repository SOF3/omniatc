use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Meta {
    /// An identifier for this map, to remain consistent over versions.
    pub id:          String,
    /// Title of the map.
    pub title:       String,
    pub description: String,
    pub authors:     Vec<String>,
    pub tags:        HashMap<String, String>,
}

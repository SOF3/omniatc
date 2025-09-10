//! Schema for save files.

use serde::{Deserialize, Serialize};

mod score;
pub use score::Score;

mod meta;
pub use meta::*;

mod level;
pub use level::*;

mod geometry;
pub use geometry::*;

mod refs;
pub use refs::*;

mod player;
pub use player::*;

mod object;
pub use object::*;

mod route;
pub use route::*;

mod ui;
pub use ui::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct File {
    /// Metadata about the file.
    pub meta:  Meta,
    /// Gameplay entities.
    pub level: Level,
    /// UI configuration.
    pub ui:    Ui,

    // The following fields are only populated for scenarios and savefiles.
    #[serde(default)]
    pub stats:   Stats,
    #[serde(default)]
    pub objects: Vec<Object>,
}

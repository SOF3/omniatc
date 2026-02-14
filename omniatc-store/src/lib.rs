//! Schema for save files.

#![forbid(missing_docs)]

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

mod object_type;
pub use object_type::*;

mod quest;
pub use quest::*;

mod route;
pub use route::*;

mod ui;
pub use ui::*;

mod weighted;
pub use weighted::*;

/// Root structure for a .osav file.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct File {
    /// Metadata about the file.
    pub meta:  Meta,
    /// Immutable elements of the game world.
    pub level: Level,
    /// The starting UI state.
    pub ui:    Ui,

    // The following fields are only populated for scenarios and savefiles.
    /// Game statistics.
    #[serde(default)]
    pub stats:   Stats,
    /// Quests to be completed.
    #[serde(default)]
    pub quests:  QuestTree,
    /// Existing objects in the level.
    #[serde(default)]
    pub objects: Vec<Object>,
}

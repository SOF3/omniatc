//! Schema for save files.

#![warn(clippy::pedantic)]
#![cfg_attr(feature = "precommit-checks", deny(warnings, clippy::pedantic, clippy::dbg_macro))]
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)] // too many false positives from system params.
#![allow(clippy::collapsible_else_if)] // this is usually intentional
#![allow(clippy::missing_panics_doc)] // 5:21 PM conrad.lock().expect("luscious")[tty0] : Worst clippy lint
#![cfg_attr(not(feature = "precommit-checks"), allow(dead_code, unused_variables, unused_imports))]
#![cfg_attr(feature = "precommit-checks", allow(dead_code))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", warn(warnings, clippy::pedantic, clippy::dbg_macro))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", allow(unused_imports))] // TODO remove this in the future
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
    /// Existing objects in the level.
    #[serde(default)]
    pub objects: Vec<Object>,
}

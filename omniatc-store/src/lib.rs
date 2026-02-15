//! Schema for save files.

#![forbid(missing_docs)]

use std::io;

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

const ZSTD_COMPRESSION_LEVEL: i32 = 3;

impl File {
    /// Serializes the file to a .osav format.
    pub fn to_osav(&self) -> Result<Vec<u8>, FileSerError> {
        let mut bytes = Vec::new();
        let mut zstd =
            zstd::Encoder::new(&mut bytes, ZSTD_COMPRESSION_LEVEL).map_err(FileSerError::Io)?;
        ciborium::into_writer(self, &mut zstd).map_err(FileSerError::Ciborium)?;
        zstd.finish().map_err(FileSerError::Io)?;
        Ok(bytes)
    }

    /// Decodes a .osav file.
    pub fn from_osav(bytes: impl io::Read) -> Result<Self, FileDeError> {
        let zstd = zstd::Decoder::new(bytes).map_err(FileDeError::Io)?;
        ciborium::from_reader(zstd).map_err(FileDeError::Ciborium)
    }
}

/// Error during serialization.
#[derive(Debug, thiserror::Error)]
pub enum FileSerError {
    /// Error from ciborium.
    #[error("cbor error: {0}")]
    Ciborium(ciborium::ser::Error<io::Error>),
    /// IO error from the writer backend during zstd header write or final flush.
    #[error("IO error: {0}")]
    Io(io::Error),
}

/// Error during deserialization.
#[derive(Debug, thiserror::Error)]
pub enum FileDeError {
    /// Error from ciborium.
    #[error("cbor error: {0}")]
    Ciborium(ciborium::de::Error<io::Error>),
    /// IO error from the reader backend during zstd header read.
    #[error("IO error: {0}")]
    Io(io::Error),
}

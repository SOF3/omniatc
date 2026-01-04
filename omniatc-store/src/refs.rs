use std::borrow::Borrow;

use serde::{Deserialize, Serialize};

/// References a runway, taxiway, or apron by label.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SegmentRef {
    /// Code of the aerodrome for the runway.
    pub aerodrome: AerodromeRef,
    /// The label of segments to be referenced.
    pub label:     SegmentLabel,
}

/// Identifies a segment within an aerodrome.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, strum::IntoStaticStr)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum SegmentLabel {
    /// Name of the taxiway.
    Taxiway(String),
    /// Name of the apron.
    Apron(String),
    /// Name of the runway (either direction).
    Runway(String),
}

impl SegmentLabel {
    /// Returns the name specified by the user to describe this label,
    /// regardless of the type of segment.
    #[must_use]
    pub fn inner_name(&self) -> &str {
        match self {
            SegmentLabel::Taxiway(name)
            | SegmentLabel::Apron(name)
            | SegmentLabel::Runway(name) => name,
        }
    }
}

/// References a position.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum WaypointRef {
    /// A regular named waypoint.
    Named(NamedWaypointRef),
    /// The threshold of a runway.
    RunwayThreshold(RunwayRef),
    /// Extended runway centerline up to localizer range,
    /// used with [`RunwayThreshold`](WaypointRef::RunwayThreshold) to represent
    /// ILS-established planes in [`TargetAlignment`](crate::TargetAlignment).
    ///
    /// For runways without a localizer, the centerline is extended up to visual range instead.
    LocalizerStart(RunwayRef),
}

/// References a regular waypoint by name.
///
/// This disallows referencing runway thresholds or localizer starts.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NamedWaypointRef(pub String);

impl From<&str> for NamedWaypointRef {
    fn from(value: &str) -> Self { Self(value.into()) }
}

/// References an aerodrome by name.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AerodromeRef(pub String);

impl From<&str> for AerodromeRef {
    fn from(value: &str) -> Self { Self(value.into()) }
}

/// References a runway.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RunwayRef {
    /// Code of the aerodrome for the runway.
    pub aerodrome:   AerodromeRef,
    /// Name of the runway.
    pub runway_name: String,
}

macro_rules! newtype_str {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        pub struct $name(pub String);

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self { Self(value.into()) }
        }
    }
}

newtype_str! {
    /// References an object type by name.
    ObjectTypeRef
}

newtype_str! {
    /// References a route preset by its `ref_id`.
    RoutePresetRef
}

newtype_str! {
    /// References a quest by ID.
    QuestRef
}

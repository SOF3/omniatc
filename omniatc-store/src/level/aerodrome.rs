use bevy_math::Vec2;
use math::{Angle, Heading, Length, Position, Speed};
use serde::{Deserialize, Serialize};

/// An aerodrome, consisting of multiple runways and ground structures.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Aerodrome {
    /// Aerodrome short display name.
    pub code:           String,
    /// Aerodrome long display name.
    pub full_name:      String,
    /// Elevation of ground structures of the aerodrome.
    pub elevation:      Position<f32>,
    /// Ground paths of the aerodrome, such as taxiways and aprons.
    pub ground_network: GroundNetwork,
    /// Runways for the aerodrome.
    pub runways:        Vec<RunwayPair>,
}

/// Ground paths of an aerodrome, such as taxiways and aprons.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GroundNetwork {
    /// Taxiways in the aerodrome.
    pub taxiways:    Vec<Taxiway>,
    /// Aprons in the aerodrome.
    pub aprons:      Vec<Apron>,
    /// Maximum speed on taxiways.
    pub taxi_speed:  Speed<f32>,
    /// Maximum speed when entering aprons.
    pub apron_speed: Speed<f32>,
}

/// A taxiway, representing any taxiable ground path
/// that is not a runway or an apron.
///
/// Taxiways automatically intersect with any other ground paths
/// that cross with it or end within one meter of it.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Taxiway {
    /// Label of the taxiway.
    ///
    /// If multiple taxiways in the same aerodrome have the same name,
    /// they are considered the same taxiway.
    pub name:      String,
    /// Points of the taxiway.
    ///
    /// Must have at least two points.
    /// A taxiway may be composed of more than two points if it is curved,
    /// in which case every two adjacent points are connected by a straight segment.
    pub endpoints: Vec<Position<Vec2>>,
    /// Width of the taxiway.
    pub width:     Length<f32>,
}

/// An apron, representing a parking area for aircraft.
///
/// Aprons automatically connect to the nearest taxiway
/// extending opposite to its forward heading.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Apron {
    /// Label of the apron.
    ///
    /// This should be unique among all aprons in the same aerodrome.
    pub name:            String,
    /// Position of aircraft when parked at the apron.
    pub position:        Position<Vec2>,
    /// Heading of aircraft when parked at the apron.
    pub forward_heading: Heading,
    /// Width of the apron.
    pub width:           Length<f32>,
}

/// A pair of opposite-direction runways.
///
/// Currently, all runways must be usable from both directions.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RunwayPair {
    /// Width of the runway. Only affects display.
    pub width:          Length<f32>,
    /// Longest takeoff starting position for the forward runway.
    pub forward_start:  Position<Vec2>,
    /// Other details of the forward runway.
    pub forward:        Runway,
    /// Longest takeoff starting position for the backward runway.
    pub backward_start: Position<Vec2>,
    /// Other details of the backward runway.
    pub backward:       Runway,
}

/// One direction of a runway.
///
/// Full runway structure: backward stopway + {forward start} + forward displacement + main +
/// backward displacement + {backward start} + forward stopway
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Runway {
    /// Runway identifier, e.g. "13R".
    ///
    /// Should not include the aerodrome name.
    pub name:                   String,
    /// Distance of the displaced threshold from runway start.
    pub touchdown_displacement: Length<f32>,
    /// Length of stopway behind the runway end (i.e. start of the opposite runway).
    pub stopway:                Length<f32>,

    /// Glide angle for the approach path.
    pub glide_angle:         Angle,
    /// Maximum distance from which the runway is visible during CAVOK conditions,
    /// allowing the aircraft to commence visual approach.
    pub max_visual_distance: Length<f32>,
    /// ILS information, if any.
    pub ils:                 Option<Localizer>,
}

/// Defines the ILS availability at a runway direction.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Localizer {
    /// An aircraft is unable to establish on ILS when
    /// the horizontal deviation from the approach path is greater than this value.
    pub half_width:       Angle,
    /// An aircraft is unable to establish on ILS when
    /// the angle of elevation above the touchdown position is less than this value.
    pub min_pitch:        Angle,
    /// An aircraft is unable to establish on ILS when
    /// the angle of elevation above the touchdown position is greater than this value.
    pub max_pitch:        Angle,
    /// An aircraft is unable to establish on ILS when
    /// the horizontal distance from the touchdown position is greater than this value.
    pub horizontal_range: Length<f32>,
    /// An aircraft is unable to establish on ILS when
    /// the vertical distance from the touchdown position is greater than this value.
    pub vertical_range:   Length<f32>,
    /// The Runway Visual Range;
    /// an aircraft must go around if visibility is lower than this value.
    pub visual_range:     Length<f32>,
    /// An aircraft must go around if it cannot establish visual contact with the runway
    /// before descending past this altitude.
    pub decision_height:  Length<f32>,
}

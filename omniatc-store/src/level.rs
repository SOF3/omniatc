use bevy_math::{Vec2, VectorSpace};
use math::{Angle, Heading, Length, Position, Speed};
use serde::{Deserialize, Serialize};

use crate::{AxisDirection, RouteNode, Shape2d, WaypointRef};

/// Contents of a map.
#[derive(Clone, Serialize, Deserialize)]
pub struct Level {
    /// Environmental features of the map.
    pub environment:   Environment,
    /// Aerodromes in the map.
    pub aerodromes:    Vec<Aerodrome>,
    /// Waypoints in the airspace.
    pub waypoints:     Vec<Waypoint>,
    /// Route presets that aircraft may be assigned to.
    pub route_presets: Vec<RoutePreset>,
}

/// Environmental features of a map.
#[derive(Clone, Serialize, Deserialize)]
pub struct Environment {
    /// Terrain altitude.
    pub heightmap: HeatMap2<Position<f32>>,

    // TODO noise abatement functions
    /// Visibility range.
    ///
    /// An object at position `P` can see an object at position `Q`
    /// if and only if both `P` and `Q` have visibility not less than `dist(P, Q)`.
    pub visibility: HeatMap2<Length<f32>>,

    /// Winds at different areas.
    pub winds: Vec<Wind>,
}

/// A 2D heatmap representing a function `Vec2 -> Datum` within a rectangle.
#[derive(Clone, Serialize, Deserialize)]
pub struct HeatMap2<Datum> {
    /// Base heatmap as a 2D dense matrix,
    /// used when majority of the terrain has irregular altitude,
    /// e.g. a real-world mountainous map or a Perlin noise generated level.
    ///
    /// For artificially generated heightmaps or heightmaps with mostly ocean,
    /// this may simply be `AlignedHeatMap2::constant(Distance(0.))`.
    pub aligned: AlignedHeatMap2<Datum>,
    /// A list of a set of R^2->R functions,
    /// used for artificially defined heatmap.
    /// The result at any point (x, y) is `functions.map(|f| f(x, y)).chain([aligned.get(x, y)]).max()`.
    pub sparse:  SparseHeatMap2<Datum>,
}

/// A 2D heatmap represented as a matrix of values of type `Datum`.
#[derive(Clone, Serialize, Deserialize)]
pub struct AlignedHeatMap2<Datum> {
    /// Coordinates of the first data point in `data`.
    pub initial_corner:  Position<Vec2>,
    /// Coordinates of the last data point in `data`.
    pub end_corner:      Position<Vec2>,
    /// The direction from `data[0]` to `data[1]`.
    pub major_direction: AxisDirection,
    /// Number of data points in one consecutive major group.
    ///
    /// `data[0]` to `data[major_length]` is exactly orthogonal to `major_direction`.
    pub major_length:    u16,
    /// Data points of the heatmap.
    ///
    /// `data[major + minor*major_length]` represents the exact height of the point
    /// `initial_corner.x.lerp(end_corner.x, major), initial_corner.y.lerp(end_corner.y, minor)`
    /// for X-major heatmaps, vice versa.
    ///
    /// A point within the AABB from `initial_corner` to `end_corner`
    /// is interpolated using the bilinear interpolation of the four closest points.
    /// A point outside the range is interpolated using the closest one or two points.
    pub data:            Vec<Datum>,
}

impl<Datum> AlignedHeatMap2<Datum> {
    /// Returns a compact heatmap representing a constant function.
    pub fn constant(value: Datum) -> Self {
        Self {
            initial_corner:  Position::new(Vec2::new(0., 0.)),
            end_corner:      Position::new(Vec2::new(0., 0.)),
            major_direction: AxisDirection::X,
            major_length:    1,
            data:            vec![value],
        }
    }

    fn minor_length(&self) -> usize { self.data.len() / usize::from(self.major_length) }

    fn minor_length_u16(&self) -> u16 {
        u16::try_from(self.minor_length()).expect("checked during validation")
    }

    fn minor_direction(&self) -> AxisDirection { self.major_direction.orthogonal() }

    /// Validate the heatmap structure.
    // TODO actually call this somewhere
    #[must_use]
    pub fn validate(&self) -> bool {
        self.major_length > 0
            && !self.data.is_empty()
            && self.data.len().is_multiple_of(usize::from(self.major_length))
            && u16::try_from(self.minor_length()).is_ok()
            && (self.major_length == 1
                || self.major_direction.of_position(self.initial_corner)
                    != self.major_direction.of_position(self.end_corner))
            && (self.minor_length() == 1
                || self.minor_direction().of_position(self.initial_corner)
                    != self.minor_direction().of_position(self.end_corner))
    }

    fn get_datum(&self, major_index: usize, minor_index: usize) -> Datum
    where
        Datum: Copy,
    {
        self.data[major_index + minor_index * usize::from(self.major_length)]
    }

    /// Resolve the function value at a given position.
    #[must_use]
    pub fn resolve(&self, position: Position<Vec2>) -> Datum
    where
        Datum: VectorSpace,
    {
        let major = self.major_direction.of_position(position).ratio_between(
            self.major_direction.of_position(self.initial_corner),
            self.major_direction.of_position(self.end_corner),
        );
        let minor = self.minor_direction().of_position(position).ratio_between(
            self.minor_direction().of_position(self.initial_corner),
            self.minor_direction().of_position(self.end_corner),
        );

        match (major, minor) {
            (0.0..=1.0, 0.0..=1.0) => {
                let major_index = major * f32::from(self.major_length - 1);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "major_index is positive"
                )]
                let major_index_floor = major_index.trunc() as usize;
                let major_index_fract = major_index.fract();
                let major_index_floor_plus_one =
                    (major_index_floor + 1).min(usize::from(self.major_length - 1));

                let minor_index = minor * f32::from(self.minor_length_u16() - 1);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "major_index is positive"
                )]
                let minor_index_floor = minor_index.trunc() as usize;
                let minor_index_fract = minor_index.fract();
                let minor_index_floor_plus_one =
                    (minor_index_floor + 1).min(self.minor_length() - 1);

                let v00 = self.get_datum(major_index_floor, minor_index_floor);
                let v10 = self.get_datum(major_index_floor_plus_one, minor_index_floor);
                let v01 = self.get_datum(major_index_floor, minor_index_floor_plus_one);
                let v11 = self.get_datum(major_index_floor_plus_one, minor_index_floor_plus_one);

                v00 * (1. - major_index_fract) * (1. - minor_index_fract)
                    + v10 * major_index_fract * (1. - minor_index_fract)
                    + v01 * (1. - major_index_fract) * minor_index_fract
                    + v11 * major_index_fract * minor_index_fract
            }
            (0.0..=1.0, _) => {
                let major_index = major * f32::from(self.major_length - 1);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "major_index is positive"
                )]
                let major_index_floor = major_index.trunc() as usize;
                let major_index_fract = major_index.fract();
                let major_index_floor_plus_one =
                    (major_index_floor + 1).min(usize::from(self.major_length - 1));

                let minor_index = if minor < 0.0 { 0 } else { self.minor_length() - 1 };

                let v0 = self.get_datum(major_index_floor, minor_index);
                let v1 = self.get_datum(major_index_floor_plus_one, minor_index);
                v0.lerp(v1, major_index_fract)
            }
            (_, 0.0..=1.0) => {
                let minor_index = minor * f32::from(self.minor_length_u16() - 1);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "major_index is positive"
                )]
                let minor_index_floor = minor_index.trunc() as usize;
                let minor_index_fract = minor_index.fract();
                let minor_index_floor_plus_one =
                    (minor_index_floor + 1).min(self.minor_length() - 1);

                let major_index = if major < 0.0 { 0 } else { self.major_length - 1 };

                let v0 = self.get_datum(major_index.into(), minor_index_floor);
                let v1 = self.get_datum(major_index.into(), minor_index_floor_plus_one);
                v0.lerp(v1, minor_index_fract)
            }
            (..0.0, ..0.0) => self.get_datum(0, 0),
            (..0.0, _) => self.get_datum(0, self.minor_length() - 1),
            (_, ..0.0) => self.get_datum((self.major_length - 1).into(), 0),
            _ => self.get_datum((self.major_length - 1).into(), self.minor_length() - 1),
        }
    }
}

/// A list of sparse functions only affecting certain areas.
#[derive(Clone, Serialize, Deserialize)]
pub struct SparseHeatMap2<Datum> {
    /// List of sparse valued areas.
    pub functions: Vec<SparseFunction2<Datum>>,
}

/// Overrides the function with a constant value when within a certain area.
#[derive(Clone, Serialize, Deserialize)]
pub struct SparseFunction2<Datum> {
    /// The area in which the function is nonzero.
    pub shape:               Shape2d,
    /// The function output within the shape.
    pub value:               Datum,
    /// Whether emergency aircraft can bypass the restriction.
    pub emergency_exception: bool,
}

/// Wind in a cuboid region, interpolated linearly between the bottom and top faces.
#[derive(Clone, Serialize, Deserialize)]
pub struct Wind {
    /// Minimum horizontal corner of the wind region.
    pub start:        Position<Vec2>,
    /// Maximum horizontal corner of the wind region.
    pub end:          Position<Vec2>,
    /// Bottom altitude of the wind region.
    pub bottom:       Position<f32>,
    /// Top altitude of the wind region.
    pub top:          Position<f32>,
    /// Wind speed at the bottom face of the region.
    pub bottom_speed: Speed<Vec2>,
    /// Wind speed at the top face of the region.
    pub top_speed:    Speed<Vec2>,
}

/// An aerodrome, consisting of multiple runways and ground structures.
#[derive(Clone, Serialize, Deserialize)]
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

/// A waypoint in the airspace.
#[derive(Clone, Serialize, Deserialize)]
pub struct Waypoint {
    /// Display name of the waypoint.
    pub name:      String,
    /// Position of the waypoint.
    pub position:  Position<Vec2>,
    /// Elevation of the navaids of the waypoint, if any.
    pub elevation: Option<Position<f32>>,
    /// Navaids provided at this waypoint.
    pub navaids:   Vec<Navaid>,
    /// Whether the waypoint can be observed visually when in proximity.
    pub visual:    Option<VisualWaypoint>,
}

/// A navigation aid provided at a waypoint.
#[derive(Clone, Serialize, Deserialize)]
pub struct Navaid {
    /// Type of navaid.
    #[serde(rename = "type")]
    pub ty:            NavaidType,
    /// Horizontal radial directions from which the navaid can be received.
    ///
    /// The range is taken in clockwise direction. That is,
    /// A receiver at heading `h` from the navaid is within this range
    /// if and only if sweeping from `heading_start` to `h` in clockwise direction
    /// does not cross `heading_end`.
    ///
    /// If `heading_start == heading_end`, there is no heading restriction.
    pub heading_start: Heading,
    /// See `heading_start`.
    pub heading_end:   Heading,

    /// Minimum angle of elevation from the navaid to the receiver.
    pub min_pitch: Angle,

    /// Maximum horizontal distance of the receiver from the navaid.
    pub max_dist_horizontal: Length<f32>,
    /// Maximum vertical distance of the receiver from the navaid.
    pub max_dist_vertical:   Length<f32>,
}

/// The type of navaid.
///
/// Currently this only affects the UI display.
#[derive(Clone, Serialize, Deserialize)]
pub enum NavaidType {
    /// This navaid tells the heading from the aircraft to the waypoint
    Vor,
    /// This navaid tells the distance of the aircraft from the waypoint.
    Dme,
}

/// Conditions under which a waypoint is visible,
/// allowing it to serve as a visual navaid.
#[derive(Clone, Serialize, Deserialize)]
pub struct VisualWaypoint {
    /// Maximum 3D distance from which pilots can see the waypoint.
    pub max_distance: Length<f32>,
}

/// A preset route that an aircraft may be assigned to.
#[derive(Clone, Serialize, Deserialize)]
pub struct RoutePreset {
    /// When is this preset available for use?
    pub trigger: RoutePresetTrigger,
    /// Identifies the preset.
    ///
    /// Different triggers of the same preset can have the same `id`,
    /// used for identifying that a route starting at another trigger
    /// can be considered equivalent when the user selects a route change.
    ///
    /// Different presets from the same trigger must not have duplicate `id`s.
    pub id:      String,
    /// Identifier used to reference the preset from other places in the save file.
    ///
    /// This field is only used in the save file and is not visible to users.
    /// If specified, it MUST be a unique value among all presets,
    /// unlike `id` which may be shared between similar presets.
    /// This field is optional and only useful when the route needs to be referenced,
    /// e.g. to initiate a goaround route.
    ///
    /// It is recommended to compose `ref_id` by appending the name of the first waypoint to `id`.
    pub ref_id:  Option<String>,
    /// Display name of this route. Not a unique identifier.
    pub title:   String,
    /// Nodes of this route.
    /// If the trigger is a waypoint,
    /// the first node should be [`DirectWaypoint`](RouteNode::DirectWaypoint) to that waypoint.
    pub nodes:   Vec<RouteNode>,
}

/// Generates [`RoutePreset`] starting at each waypoint on the way.
#[must_use]
pub fn route_presets_at_waypoints(
    id: &str,
    title: &str,
    nodes: Vec<RouteNode>,
) -> Vec<RoutePreset> {
    nodes
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(start_index, start_node)| {
            let RouteNode::DirectWaypoint {
                waypoint: waypoint @ WaypointRef::Named(waypoint_name),
                ..
            } = start_node
            else {
                return None;
            };
            Some(RoutePreset {
                trigger: RoutePresetTrigger::Waypoint(waypoint.clone()),
                id:      id.to_owned(),
                ref_id:  Some(format!("{id} {waypoint_name}")),
                title:   title.to_owned(),
                nodes:   nodes[start_index..].to_vec(),
            })
        })
        .collect()
}

/// Defines when a route preset may be selected.
#[derive(Clone, Serialize, Deserialize)]
pub enum RoutePresetTrigger {
    /// This preset may be selected when the current direct target is the waypoint.
    Waypoint(WaypointRef),
}

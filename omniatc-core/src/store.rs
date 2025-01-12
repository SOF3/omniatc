use std::time::Duration;

use bevy::math::Vec2;
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::level::{nav, plane};
use crate::units::{Accel, Angle, AngularSpeed, Distance, Heading, Position, Speed};

pub mod load;

/// Marks that an entity was loaded from a save file, and should be deleted during reload.
#[derive(Component)]
pub struct LoadedEntity;

#[derive(Clone, Serialize, Deserialize)]
pub struct File {
    /// Metadata about the file.
    pub meta:  Meta,
    /// Gameplay entities.
    pub level: Level,
    /// UI configuration.
    pub ui:    Ui,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Meta {
    /// Title of the map.
    pub title:       String,
    pub description: String,
    pub authors:     Vec<String>,
    pub tags:        Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Level {
    pub environment: Environment,
    pub aerodromes:  Vec<Aerodrome>,
    pub waypoints:   Vec<Waypoint>,
    #[serde(default)]
    pub objects:     Vec<Object>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Environment {
    /// Terrain altitude.
    pub heightmap: HeatMap2<Position<f32>>,

    // TODO noise abatement functions
    /// Visibility range.
    ///
    /// An object at position `P` can see an object at position `Q`
    /// if and only if both `P` and `Q` have visibility not less than `dist(P, Q)`.
    pub visibility: HeatMap2<Distance<f32>>,

    pub winds: Vec<Wind>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HeatMap2<T> {
    /// Base heatmap as a 2D dense matrix,
    /// used when majority of the terrain has irregular altitude,
    /// e.g. a real-world mountainous map
    /// or a Perlin noise generated level.
    ///
    /// For artificially generated heightmaps or heightmaps with mostly ocean,
    /// this may simply be `AlignedHeatMap2::constant(Distance(0.))`.
    pub aligned: AlignedHeatMap2<T>,
    /// A list of a set of R^2->R functions,
    /// used for artificially defined heatmap.
    /// The result at any point (x, y) is `functions.map(|f| f(x, y)).chain([aligned.get(x, y)]).max()`.
    pub sparse:  SparseHeatMap2<T>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AlignedHeatMap2<Datum> {
    /// Coordinates of the first data point in `data`.
    pub initial_corner:  Position<Vec2>,
    /// Coordinates of the last data point in `data`.
    pub end_corner:      Position<Vec2>,
    /// The direction from `data[0]` to `data[1]`.
    pub major_direction: AxisDirection,
    /// Number of data points in one consecutive major group.
    pub major_length:    u32,
    /// Data points of the heatmap.
    ///
    /// `data[major + minor*major_length]` represents the exact height of the point
    /// `initial_corner.x.lerp(end_corner.x, major), initial_corner.y.lerp(end_corner.y, minor)`
    /// for X-major heatmaps, vice versa.
    ///
    /// A point between the AABB from `initial_corner` to `end_corner`
    /// is interpolated using the closest three points.
    /// A point outside the range is interpolated using the closest one or two points.
    pub data:            Vec<Datum>,
}

impl<Datum> AlignedHeatMap2<Datum> {
    pub fn constant(value: Datum) -> Self {
        Self {
            initial_corner:  Position::new(Vec2::new(0., 0.)),
            end_corner:      Position::new(Vec2::new(0., 0.)),
            major_direction: AxisDirection::X,
            major_length:    1,
            data:            vec![value],
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SparseHeatMap2<Datum> {
    /// List of sparse valued areas.
    pub functions: Vec<SparseFunction2<Datum>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SparseFunction2<Datum> {
    /// The area in which the function is nonzero.
    pub shape:               Shape2d,
    /// The function output within the shape.
    pub value:               Datum,
    /// Whether emergency aircraft can bypass the restriction.
    pub emergency_exception: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Wind {
    pub start:        Position<Vec2>,
    pub end:          Position<Vec2>,
    pub bottom:       Position<f32>,
    pub top:          Position<f32>,
    pub bottom_speed: Speed<Vec2>,
    pub top_speed:    Speed<Vec2>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Aerodrome {
    /// Aerodrome short display name.
    pub code:      String,
    /// Aerodrome long display name.
    pub full_name: String,
    /// Runways for the aerodrome.
    pub runways:   Vec<Runway>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Runway {
    /// Runway identifier, e.g. "13R".
    ///
    /// Should not include the aerodrome name.
    pub name:                       String,
    /// Elevation of the runway.
    pub elevation:                  Position<f32>,
    /// Position of the touchdown marker.
    pub touchdown_position:         Position<Vec2>,
    /// Heading of the runway.
    pub heading:                    Heading,
    /// Declared landing distance available.
    /// Extended from touchdown position in the runway heading direction.
    pub landing_distance_available: Distance<f32>,
    /// Length of the displaced threshold.
    /// The actual runway length is extended *behind* the touchdown position
    /// for this length.
    pub touchdown_displacement:     Distance<f32>,
    /// Glide angle for the approach path.
    pub glide_angle:                Angle<f32>,
    /// Width of the runway. Only affects display.
    pub width:                      Distance<f32>,
    /// Maximum distance from which the runway is visible during CAVOK conditions,
    /// allowing the aircraft to commence visual approach.
    pub max_visual_distance:        Distance<f32>,

    /// ILS information, if any.
    pub ils: Option<Localizer>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Localizer {
    /// An aircraft is unable to establish on ILS when
    /// the horizontal deviation from the approach path is greater than this value.
    pub half_width:       Angle<f32>,
    /// An aircraft is unable to establish on ILS when
    /// the angle of elevation above the touchdown position is less than this value.
    pub min_pitch:        Angle<f32>,
    /// An aircraft is unable to establish on ILS when
    /// the angle of elevation above the touchdown position is greater than this value.
    pub max_pitch:        Angle<f32>,
    /// An aircraft is unable to establish on ILS when
    /// the horizontal distance from the touchdown position is greater than this value.
    pub horizontal_range: Distance<f32>,
    /// An aircraft is unable to establish on ILS when
    /// the vertical distance from the touchdown position is greater than this value.
    pub vertical_range:   Distance<f32>,
    /// The Runway Visual Range;
    /// an aircraft must go around if visibility is lower than this value.
    pub visual_range:     Distance<f32>,
    /// An aircraft must go around if it cannot establish visual contact with the runway
    /// before descending past this altitude.
    pub decision_height:  Distance<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Waypoint {
    /// Display name of the waypoint.
    pub name:      String,
    /// Position of the waypoint.
    pub position:  Position<Vec2>,
    /// Elevation of the navaids of the waypoint, if any.
    pub elevation: Option<Position<f32>>,
    /// Navaids available at this waypoint.
    pub navaids:   Vec<Navaid>,
    /// Whether the waypoint can be observed visually when in proximity.
    pub visual:    Option<VisualWaypoint>,
}

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
    pub min_pitch: Angle<f32>,

    /// Maximum horizontal distance of the receiver from the navaid.
    pub max_dist_horizontal: Distance<f32>,
    /// Maximum vertical distance of the receiver from the navaid.
    pub max_dist_vertical:   Distance<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum NavaidType {
    /// This navaid tells the heading from the aircraft to the waypoint
    Vor,
    /// This navaid tells the distance of the aircraft from the waypoint.
    Dme,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VisualWaypoint {
    /// Maximum 3D distance from which pilots can see the waypoint.
    pub max_distance: Distance<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Object {
    Plane(Plane),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Plane {
    pub aircraft:     BaseAircraft,
    pub control:      PlaneControl,
    #[allow(clippy::struct_field_names)]
    pub plane_limits: plane::Limits,
    pub nav_limits:   nav::Limits,
    pub nav_target:   NavTarget,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BaseAircraft {
    pub name:         String,
    pub dest:         Destination, // TODO
    pub position:     Position<Vec2>,
    pub altitude:     Position<f32>,
    /// Speed of ground projection displacement.
    pub ground_speed: Speed<f32>,
    /// Direction of ground projection displacement.
    pub ground_dir:   Heading,
    pub vert_rate:    Speed<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Destination {
    Departure { aerodrome_code: String, dest_waypoint: String },
    Arrival { aerodrome_code: String },
    Ferry { source_aerodrome_code: String, dest_aerodrome_code: String },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PlaneControl {
    pub heading:     Heading,
    pub yaw_speed:   AngularSpeed<f32>,
    pub horiz_accel: Accel<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum NavTarget {
    Airborne(Box<AirborneNavTarget>),
    Ground(GroundNavTarget),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AirborneNavTarget {
    /// Target yaw change.
    pub yaw:         nav::YawTarget,
    /// Target horizontal indicated airspeed.
    pub horiz_speed: Speed<f32>,
    /// Target vertical rate.
    pub vert_rate:   Speed<f32>,
    /// Whether vertical rate should be expedited.
    /// If false, `vert_rate` is clamped by normal rate instead of the expedition rate.
    pub expedite:    bool,

    pub target_altitude:  Option<TargetAltitude>,
    pub target_glide:     Option<TargetGlide>,
    pub target_waypoint:  Option<TargetWaypoint>,
    pub target_alignment: Option<TargetAlignment>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GroundNavTarget {
    pub velocity: nav::VelocityTarget,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TargetAltitude {
    /// Altitude to move towards and maintain.
    pub altitude: Position<f32>,
    /// Whether to expedite towards the altitude.
    pub expedite: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TargetGlide {
    /// Target waypoint to aim at.
    pub target_waypoint: WaypointRef,
    /// Angle of depression of the glide path. Typically negative.
    pub glide_angle:     Angle<f32>,
    /// Most negative pitch to use.
    pub min_pitch:       Angle<f32>,
    /// Highest pitch to use.
    pub max_pitch:       Angle<f32>,
    /// Lookahead time for pure pursuit.
    pub lookahead:       Duration,
    /// Whether the aircraft should expedit climb/descent to intersect with the glidepath.
    ///
    /// If false, the min/max pitch is further restricted by the standard climb/descent rate.
    /// If true, it is only restricted by the expedition rate (which would be the physical limit).
    pub expedite:        bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TargetWaypoint {
    /// Name of target waypoint.
    pub waypoint: WaypointRef,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TargetAlignment {
    /// Name of start waypoint.
    pub start_waypoint:   WaypointRef,
    /// Name of end waypoint.
    pub end_waypoint:     WaypointRef,
    /// Lookahead time for pure pursuit.
    pub lookahead:        Duration,
    /// Maximum orthogonal distance between the line and the object
    /// within which direction control is activated for alignment.
    /// This is used to avoid prematurely turning directly towards the localizer.
    pub activation_range: Distance<f32>,
}

/// References a position.
#[derive(Clone, Serialize, Deserialize)]
pub enum WaypointRef {
    /// A regular named waypoint.
    Named(String),
    /// The threshold of a runway.
    RunwayThreshold(RunwayRef),
    /// Extended runway centerline up to localizer range,
    /// used with [`RunwayThreshold`](WaypointRef::RunwayThreshold) to represent
    /// ILS-established planes in [`TargetAlignment`].
    ///
    /// For runways without a localizer, the centerline is extended up to visual range instead.
    LocalizerStart(RunwayRef),
}

/// References a runway.
#[derive(Clone, Serialize, Deserialize)]
pub struct RunwayRef {
    /// Code of the aerodrome for the runway.
    pub aerodrome_code: String,
    /// Name of the runway.
    pub runway_name:    String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Ui {
    pub camera: Camera,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Camera {
    TwoDimension(Camera2d),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Camera2d {
    /// Level position that the camera is centered in.
    pub center: Position<Vec2>,

    /// Heading of the upward direction of the camera.
    /// 0 degrees means north is upwards; 90 degrees means east is upwards.
    pub up: Heading,

    /// Whether the camera scale is based on X (width) or Y (height) axis.
    pub scale_axis:   AxisDirection,
    /// Number of nautical miles to display in the scale axis.
    pub scale_length: Distance<f32>,
}

/// A horizontal map axis.
#[derive(Clone, Serialize, Deserialize)]
pub enum AxisDirection {
    X,
    Y,
}

/// A 2D shape.
#[derive(Clone, Serialize, Deserialize)]
pub enum Shape2d {
    Ellipse {
        /// Center of the ellipse.
        center:       Position<Vec2>,
        /// Length of the major axis.
        major_radius: Distance<f32>,
        /// Length of the minor axis.
        minor_radius: Distance<f32>,
        /// Direction of the major axis.
        major_dir:    Angle<f32>,
    },
    Polygon {
        points: Vec<Position<Vec2>>,
    },
}

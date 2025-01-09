use std::time::Duration;

use bevy::math::Vec2;
use serde::{Deserialize, Serialize};

use crate::level::{nav, plane};
use crate::units::{Accel, Angle, AngularSpeed, Distance, Position, Speed};

#[derive(Serialize, Deserialize)]
pub struct File {
    pub level: Level,
    pub ui:    Ui,
}

#[derive(Serialize, Deserialize)]
pub struct Level {
    pub environment: Environment,
    pub aerodromes:  Vec<Aerodrome>,
    pub waypoints:   Vec<Waypoint>,
    #[serde(default)]
    pub objects:     Vec<Object>,
}

#[derive(Serialize, Deserialize)]
pub struct Environment {
    /// Terrain altitude.
    pub heightmap: HeatMap2<Distance<f32>>,

    // TODO noise abatement functions
    /// Visibility range.
    ///
    /// An object at position `P` can see an object at position `Q`
    /// if and only if both `P` and `Q` have visibility not less than `dist(P, Q)`.
    pub visibility: HeatMap2<Distance<f32>>,
    // pub wind: HeatMap3<Speed<f32>>, // TODO
}

#[derive(Serialize, Deserialize)]
pub struct HeatMap2<T> {
    /// Base heatmap as a 2D dense matrix,
    /// used when majority of the terrain has irregular altitude,
    /// e.g. a real-world mountainous map
    /// or a Perlin noise generated level.
    ///
    /// For artificially generated maps or maps with mostly ocean,
    /// this field may be omitted, which defaults to a zero heatmap.
    aligned: Option<AlignedHeatMap2<T>>,
    /// A list of a set of R^2->R functions,
    /// used for artificially defined heatmap.
    /// The result at any point (x, y) is `functions.map(|f| f(x, y)).chain([aligned.get(x, y)]).max()`.
    sparse:  SparseHeatMap2<T>,
}

#[derive(Serialize, Deserialize)]
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

impl<Datum: Default> Default for AlignedHeatMap2<Datum> {
    fn default() -> Self {
        Self {
            initial_corner:  Position::new(Vec2::new(0., 0.)),
            end_corner:      Position::new(Vec2::new(0., 0.)),
            major_direction: AxisDirection::X,
            major_length:    1,
            data:            vec![Datum::default()],
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SparseHeatMap2<Datum> {
    /// List of sparse valued areas.
    pub functions: Vec<SparseFunction2<Datum>>,
}

#[derive(Serialize, Deserialize)]
pub struct SparseFunction2<Datum> {
    /// The area in which the function is nonzero.
    pub shape:               Shape2d,
    /// The function output within the shape.
    pub value:               Datum,
    /// Whether emergency aircraft can bypass the restriction.
    pub emergency_exception: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Aerodrome {
    /// Aerodrome short display name.
    pub name:      String,
    /// Aerodrome long display name.
    pub full_name: String,
    /// Runways for the aerodrome.
    pub runways:   Vec<Runway>,
}

#[derive(Serialize, Deserialize)]
pub struct Runway {
    /// Runway display name, e.g. "13R".
    pub name:                       String,
    /// Elevation of the runway.
    pub elevation:                  Distance<f32>,
    /// Position of the touchdown marker.
    pub touchdown_position:         Position<Vec2>,
    /// Heading of the runway.
    pub heading:                    Angle<f32>,
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

    /// ILS information, if any.
    pub ils: Option<Localizer>,
}

#[derive(Serialize, Deserialize)]
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
}

#[derive(Serialize, Deserialize)]
pub struct Waypoint {
    /// Display name of the waypoint.
    pub name:      String,
    /// Position of the waypoint.
    pub position:  Position<Vec2>,
    /// Elevation of the navaids of the waypoint, if any.
    pub elevation: Option<Distance<f32>>,
    /// Navaids available at this waypoint.
    pub navaids:   Vec<Navaid>,
    /// Whether the waypoint can be observed visually when in proximity.
    pub visual:    Option<VisualWaypoint>,
}

#[derive(Serialize, Deserialize)]
pub struct Navaid {
    /// Type of navaid.
    #[serde(rename = "type")]
    pub ty: NavaidType,
}

#[derive(Serialize, Deserialize)]
pub enum NavaidType {
    /// This navaid tells the heading from the aircraft to the waypoint
    Vor,
    /// This navaid tells the distance of the aircraft from the waypoint.
    Dme,
}

#[derive(Serialize, Deserialize)]
pub struct VisualWaypoint {
    /// Maximum 3D distance from which pilots can see the waypoint.
    pub max_distance: Distance<f32>,
}

#[derive(Serialize, Deserialize)]
pub enum Object {
    Plane(Plane),
}

#[derive(Serialize, Deserialize)]
pub struct Plane {
    pub aircraft:        BaseAircraft,
    pub control:         PlaneControl,
    #[allow(clippy::struct_field_names)]
    pub plane_limits:    plane::Limits,
    pub nav_limits:      nav::Limits,
    pub airborne:        Option<Airborne>,
    pub velocity_target: nav::VelocityTarget,
}

#[derive(Serialize, Deserialize)]
pub struct BaseAircraft {
    pub name:         String,
    // pub dest: Destination, // TODO
    pub position:     Position<Vec2>,
    pub altitude:     Distance<f32>,
    /// Speed of ground projection displacement.
    pub ground_speed: Speed<f32>,
    /// Direction of ground projection displacement.
    pub ground_dir:   Angle<f32>,
    pub is_airborne:  bool,
}

#[derive(Serialize, Deserialize)]
pub struct PlaneControl {
    pub heading:     Angle<f32>,
    pub yaw_speed:   AngularSpeed<f32>,
    pub horiz_accel: Accel<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct Airborne {
    pub velocity:         nav::VelocityTarget,
    pub target_altitude:  Option<nav::TargetAltitude>,
    pub target_waypoint:  Option<TargetWaypoint>,
    pub target_alignment: Option<TargetAlignment>,
}

#[derive(Serialize, Deserialize)]
pub struct TargetWaypoint {
    /// Name of target waypoint.
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct TargetAlignment {
    /// Name of start waypoint.
    pub start_waypoint:   String,
    /// Name of end waypoint.
    pub end_waypoint:     String,
    /// Lookahead time for pure pursuit.
    pub lookahead:        Duration,
    /// Maximum orthogonal distance between the line and the object
    /// within which direction control is activated for alignment.
    /// This is used to avoid prematurely turning directly towards the localizer.
    pub activation_range: Distance<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct Ui {
    pub camera: Camera,
}

#[derive(Serialize, Deserialize)]
pub struct Camera {
    /// Level position that the camera is centered in.
    pub center: Position<Vec2>,

    /// Heading of the camera in degrees.
    /// 0 means north is upwards; 90 means east is upwards.
    pub up: Angle<f32>,

    /// Whether the camera scale is based on X (width) or Y (height) axis.
    pub scale_axis:   AxisDirection,
    /// Number of nautical miles to display in the scale axis.
    pub scale_length: Distance<f32>,
}

/// A horizontal map axis.
#[derive(Serialize, Deserialize)]
pub enum AxisDirection {
    X,
    Y,
}

/// A 2D shape.
#[derive(Serialize, Deserialize)]
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

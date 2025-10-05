use bevy_math::{Vec2, VectorSpace};
use math::{Length, Position, Speed};
use serde::{Deserialize, Serialize};

use crate::{AxisDirection, Shape2d};

/// Environmental features of a map.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
        Datum: VectorSpace<Scalar = f32>,
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SparseHeatMap2<Datum> {
    /// List of sparse valued areas.
    pub functions: Vec<SparseFunction2<Datum>>,
}

/// Overrides the function with a constant value when within a certain area.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

//! Universal constants related to physics and units.

// we don't really want to read the mathematical constants in this file.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use std::{cmp, fmt, iter, ops};

use bevy::math::{Dir2, Mat2, Vec2};

use crate::units::{Distance, Position, Speed, Squared, TurnDirection};

mod consts;
pub use consts::*;
pub mod sweep;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sign {
    Negative,
    Zero,
    Positive,
}

/// Finds the values `k1`, `k2` such that `0 <= k1 <= k2 <= 1` and,
/// for each `k` between `k1` and `k2` inclusive,
/// `line_start.lerp(line_end, k).distance(circle_center) <= radius`.
///
/// Returns None if the circle does not intersect with the line segment.
/// The output is always a subset of `0..=1`.
#[must_use]
pub fn line_circle_intersect(
    circle_center: Position<Vec2>,
    radius_sq: Squared<Distance<f32>>,
    line_start: Position<Vec2>,
    line_end: Position<Vec2>,
) -> Option<[f32; 2]> {
    let line_dir = line_end - line_start;
    let center_to_start = line_start - circle_center;

    // Quadratic form of center_to_start + k * line_dir
    // in terms of a * k^2 + b * k + c = 0
    let a = line_dir.x().squared() + line_dir.y().squared();
    let b = (line_dir.x() * center_to_start.x() + line_dir.y() * center_to_start.y()) * 2.;
    let c = center_to_start.x().squared() + center_to_start.y().squared() - radius_sq;

    let discrim = b.squared() - a * c * 4.;
    if discrim.is_negative() {
        None
    } else {
        let low = ((-b - discrim.sqrt()) / a / 2.).max(0.);
        let high = ((-b + discrim.sqrt()) / a / 2.).min(1.);
        Some([low, high]).filter(|_| low <= high)
    }
}

pub trait Between<U>: PartialOrd<U> {
    fn between_inclusive(&self, min: &U, max: &U) -> bool { self >= min && self <= max }
}

impl<T: PartialOrd<U>, U> Between<U> for T {}

#[must_use]
pub fn solve_expected_ground_speed(
    true_airspeed: Speed<f32>,
    wind: Speed<Vec2>,
    ground_dir: Dir2,
) -> Speed<f32> {
    let wind_dot_ground = wind.x() * ground_dir.x + wind.y() * ground_dir.y;
    wind_dot_ground
        + (true_airspeed.squared() - wind.magnitude_squared() - wind_dot_ground.squared()).sqrt()
}

/// Returns `start`, `start+interval`, `start+interval+interval`, ... until `end`.
/// The second last item is between `end - interval` and `end`, and is not equal to `end`.
///
/// # Panics
/// Panics if `interval` is not a finite positive or negative value.
pub fn range_steps<T, U>(mut start: T, end: T, interval: U) -> impl Iterator<Item = T> + Clone
where
    T: Copy + PartialOrd + ops::AddAssign<U>,
    U: fmt::Debug + Copy + Default + PartialOrd,
{
    let more_extreme = match interval.partial_cmp(&U::default()) {
        Some(cmp::Ordering::Less) => |a: T, b: T| a <= b,
        Some(cmp::Ordering::Greater) => |a, b| a >= b,
        _ => panic!("interval {interval:?} must be a finite positive or negative"),
    };

    let mut fuse = Some(end).filter(|_| more_extreme(end, start));

    iter::from_fn(move || {
        let output = start;
        if more_extreme(output, end) {
            fuse.take()
        } else {
            start += interval;
            Some(output)
        }
    })
}

/// Solve `(t1, t2)` for `s1 + d1 * t1 == s2 + d2 * t2`.
#[must_use]
pub fn line_intersect(s1: Vec2, d1: Vec2, s2: Vec2, d2: Vec2) -> (f32, f32) {
    let mat = Mat2::from_cols(d1, -d2);
    let t = mat.inverse() * (s2 - s1);
    (t.x, t.y)
}

/// Returns the closest point from `point` on the extended line intersecting `line_start` and `line_end`
#[must_use]
pub fn point_line_closest(
    point: Position<Vec2>,
    line_start: Position<Vec2>,
    line_end: Position<Vec2>,
) -> Position<Vec2> {
    let line_dir = line_end - line_start;
    let ortho_dir = line_dir.rotate_right_angle_clockwise();

    let (line_t, _ortho_t) = line_intersect(line_start.get(), line_dir.0, point.get(), ortho_dir.0);
    line_start + line_dir * line_t
}

/// Returns the two points on the circle at `center` with radius `radius`
/// such that the tangent of the circle at each point intersects with `outside`.
///
/// Returns `None` if `outside` is inside the circle.
#[must_use]
pub fn find_circle_tangents_intersecting(outside: Vec2, radius: f32) -> Option<[Vec2; 2]> {
    // Solve radial.dot(radial - shifted) = 0 for radial, which reduces to a quadratic equation:
    let a = outside.length_squared();
    let b = -2.0 * outside.x * radius.powi(2);
    let c = radius.powi(4) - (outside.y * radius).powi(2);

    let discrim = b.powi(2) - 4.0 * a * c;
    if discrim <= 0. {
        None
    } else {
        let low = (-b - discrim.sqrt()) / a / 2.;
        let high = (-b + discrim.sqrt()) / a / 2.;
        Some([low, high].map(|x| {
            // Solve radius^2 = radial.dot(shifted) for radial.y
            let y = (radius.powi(2) - outside.x * x) / outside.y;
            Vec2 { x, y }
        }))
    }
}

#[must_use]
pub fn find_circle_tangent_towards(
    outside: Position<Vec2>,
    center: Position<Vec2>,
    radius: Distance<f32>,
    direction: TurnDirection,
) -> Option<Position<Vec2>> {
    let direct = outside - center;
    let radials = find_circle_tangents_intersecting(direct.0, radius.0)?.map(Distance);
    if TurnDirection::from_triangle_23(radials[0], direct) == Some(direction) {
        Some(center + radials[0])
    } else {
        Some(center + radials[1])
    }
}

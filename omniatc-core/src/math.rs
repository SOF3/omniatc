//! Universal constants related to physics and units.

// we don't really want to read the mathematical constants in this file.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use core::fmt;
use std::{cmp, iter, ops};

use bevy::math::{Dir2, Vec2};

mod consts;
pub use consts::*;

use crate::units::{Distance, Position, Speed, Squared};

#[cfg(test)]
mod tests;

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
/// The second last item is greater than or equal to `end - interval` and not equal to `end`.
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

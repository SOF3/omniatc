//! Universal constants related to physics and units.

// we don't really want to read the mathematical constants in this file.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use bevy::math::Vec2;

mod consts;
pub use consts::*;

use crate::units::{Distance, Position, Squared};

#[cfg(test)]
mod tests;

/// Finds the values `k1`, `k2` such that `0 <= k1 <= k2 <= 1` and,
/// for each `k` between `k1` and `k2` inclusive,
/// `line_start.lerp(line_end, k).distance(circle_center) <= radius`.
///
/// Returns None if the circle does not intersect with the line segment.
/// The output is always a subset of `0..=1`.
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

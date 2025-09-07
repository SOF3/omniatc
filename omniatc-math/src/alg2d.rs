//! Simple 2D coordinate geometry and linear algebra algorithms.

use bevy_math::{Mat2, Vec2};

use crate::{CanSqrt, Length, Position, Squared, TurnDirection};

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
    radius_sq: Squared<Length<f32>>,
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

/// Solve `(t1, t2)` for `s1 + d1 * t1 == s2 + d2 * t2`.
#[must_use]
pub fn line_intersect(s1: Vec2, d1: Vec2, s2: Vec2, d2: Vec2) -> (f32, f32) {
    let mat = Mat2::from_cols(d1, -d2);
    let t = mat.inverse() * (s2 - s1);
    (t.x, t.y)
}

/// Returns the closest point from `point` on the extended line intersecting `line_start` and `line_end`.
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

/// Returns the closest point from `point` on the line segment between `line_start` and `line_end`.
#[must_use]
pub fn point_line_segment_closest(
    point: Position<Vec2>,
    line_start: Position<Vec2>,
    line_end: Position<Vec2>,
) -> Position<Vec2> {
    let line_dir = line_end - line_start;
    let ortho_dir = line_dir.rotate_right_angle_clockwise();

    let (line_t, _ortho_t) = line_intersect(line_start.get(), line_dir.0, point.get(), ortho_dir.0);
    line_start + line_dir * line_t.clamp(0.0, 1.0)
}

/// Returns the shortest distance between two line segments.
///
/// The result is the vector between the two closest points on each segment
/// from segment 0 to segment 1.
///
/// An exactly zero vector is guaranteed when the two segments intersect.
#[must_use]
pub fn segment_segment_distance(
    start_0: Position<Vec2>,
    dir_0: Length<Vec2>,
    start_1: Position<Vec2>,
    dir_1: Length<Vec2>,
) -> Length<Vec2> {
    let (t0, t1) = line_intersect(start_0.get(), dir_0.0, start_1.get(), dir_1.0);
    if (0.0..=1.0).contains(&t0) && (0.0..=1.0).contains(&t1) {
        // Always return exact zero to avoid floating point precision issues.
        Length::ZERO
    } else {
        let p0 = start_0 + dir_0 * t0.clamp(0.0, 1.0);
        let p1 = start_1 + dir_1 * t1.clamp(0.0, 1.0);
        p1 - p0
    }
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
    radius: Length<f32>,
    direction: TurnDirection,
) -> Option<Position<Vec2>> {
    let direct = outside - center;
    let radials = find_circle_tangents_intersecting(direct.0, radius.0)?.map(Length::new);
    if TurnDirection::from_triangle_23(radials[0], direct) == Some(direction) {
        Some(center + radials[0])
    } else {
        Some(center + radials[1])
    }
}

/// Rotate a vector 90 degrees clockwise.
#[must_use]
pub fn rotate_clockwise(v: Vec2) -> Vec2 { Vec2 { x: v.y, y: -v.x } }

/// Rotate a vector 90 degrees counterclockwise.
#[must_use]
pub fn rotate_counterclockwise(v: Vec2) -> Vec2 { Vec2 { x: -v.y, y: v.x } }

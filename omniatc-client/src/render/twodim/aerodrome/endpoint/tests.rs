use bevy::math::Vec2;
use itertools::{Itertools, MinMaxResult};
use math::{Angle, Length, Position};

const DISTANCE_EPSILON: Length<f32> = Length::from_nm(0.01);

#[test]
fn test_compute_curve_parts() {
    let center = Position::from_origin_nm(100., 100.);

    let start = center + Length::vec2_from_nm(Vec2::new(1., 0.));
    let end = center + Length::vec2_from_nm(Vec2::new(1., 1.));

    let curve_points: Vec<_> = super::compute_curve_points(
        center,
        [start, end],
        Length::from_nm(1.),
        Angle::from_degrees(15.),
    )
    .collect();

    for &point in &curve_points {
        assert!(point.distance_cmp(center) < Length::from_nm(1.1));
    }

    assert!(curve_points.first().unwrap().distance_cmp(start) < DISTANCE_EPSILON);
    assert!(
        curve_points
            .last()
            .unwrap()
            .distance_cmp(center + (end - center).normalize_to_magnitude(Length::from_nm(1.)))
            < DISTANCE_EPSILON
    );

    let segments: Vec<_> =
        curve_points.into_iter().tuple_windows().map(|(from, to)| to - from).collect();

    let minmax = segments.iter().copied().map(Length::magnitude_exact).minmax();
    assert!(matches!(minmax, MinMaxResult::MinMax(min, max) if max - min < DISTANCE_EPSILON));

    for (h1, h2) in segments.into_iter().map(Length::heading).tuple_windows() {
        assert!((h2 - h1) - Angle::from_degrees(15.) < Angle::from_degrees(0.01));
    }
}

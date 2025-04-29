use bevy::math::Vec2;
use itertools::{Itertools, MinMaxResult};
use omniatc_core::units::{Angle, Distance, Position};

#[test]
fn test_compute_curve_parts() {
    let center = Position::from_origin_nm(100., 100.);

    let start = center + Distance(Vec2::new(1., 0.));
    let end = center + Distance(Vec2::new(1., 1.));

    let curve_points: Vec<_> = super::compute_curve_points(
        center,
        [start, end],
        Distance::from_nm(1.),
        Angle::from_degrees(15.),
    )
    .collect();

    for &point in &curve_points {
        assert!(point.distance_cmp(center) < Distance::from_nm(1.1));
    }

    assert!(curve_points.first().unwrap().distance_cmp(start) < Distance(0.01));
    assert!(
        curve_points
            .last()
            .unwrap()
            .distance_cmp(center + (end - center).normalize_to_magnitude(Distance::from_nm(1.)))
            < Distance(0.01)
    );

    let segments: Vec<_> =
        curve_points.into_iter().tuple_windows().map(|(from, to)| to - from).collect();

    let minmax = segments.iter().copied().map(Distance::magnitude_exact).minmax();
    assert!(matches!(minmax, MinMaxResult::MinMax(min, max) if max - min < Distance(0.01)));

    for (h1, h2) in segments.into_iter().map(Distance::heading).tuple_windows() {
        assert!((h2 - h1) - Angle::from_degrees(15.) < Angle::from_degrees(0.01));
    }
}

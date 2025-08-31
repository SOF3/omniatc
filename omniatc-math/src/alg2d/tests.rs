use bevy_math::Vec2;

use crate::{
    Length, Position, Quantity, TurnDirection, find_circle_tangent_towards, line_circle_intersect,
};

fn assert_line_circle_intersect(actual: Option<[f32; 2]>, expect: Option<[f32; 2]>) {
    assert_eq!(actual.is_none(), expect.is_none());

    if let (Some([actual_low, actual_high]), Some([expect_low, expect_high])) = (actual, expect) {
        assert!(
            (actual_low - expect_low).abs() < 1e-5,
            "expected k1 = {expect_low:?}, got {actual_low:?}"
        );
        assert!(
            (actual_high - expect_high).abs() < 1e-5,
            "expected k2 = {expect_high:?}, got {actual_high:?}"
        );
    }
}

#[test]
fn line_circle_intersect_middle() {
    let line_length = 200f32.sqrt();
    let radius_ratio = 2. / line_length;

    assert_line_circle_intersect(
        line_circle_intersect(
            Position::new(Vec2::new(10., 0.)),
            Quantity::new(4.),
            Position::new(Vec2::new(5., 5.)),
            Position::new(Vec2::new(15., -5.)),
        ),
        Some([0.5 - radius_ratio, 0.5 + radius_ratio]),
    );
}

#[test]
fn line_circle_intersect_contain_start() {
    assert_line_circle_intersect(
        line_circle_intersect(
            Position::new(Vec2::new(10., 0.)),
            Quantity::new(4.),
            Position::new(Vec2::new(9., 1.)),
            Position::new(Vec2::new(19., 1.)),
        ),
        Some([0., (3f32.sqrt() + 1.) / 10.]),
    );
}

#[test]
fn line_circle_intersect_contain_end() {
    assert_line_circle_intersect(
        line_circle_intersect(
            Position::new(Vec2::new(10., 0.)),
            Quantity::new(4.),
            Position::new(Vec2::new(1., 1.)),
            Position::new(Vec2::new(11., 1.)),
        ),
        Some([1. - (3f32.sqrt() + 1.) / 10., 1.]),
    );
}

#[test]
fn line_circle_intersect_outside() {
    assert_line_circle_intersect(
        line_circle_intersect(
            Position::new(Vec2::new(10., 0.)),
            Quantity::new(4.),
            Position::new(Vec2::new(10., 10.)),
            Position::new(Vec2::new(0., 0.)),
        ),
        None,
    );
}

fn assert_option_pos(actual: Option<Position<Vec2>>, expect: Option<Position<Vec2>>) {
    assert_eq!(actual.is_some(), expect.is_some());
    if let (Some(actual), Some(expect)) = (actual, expect) {
        assert!(
            actual.distance_cmp(expect) < Length::from_nm(0.001),
            "expect {expect:?}, got {actual:?}"
        );
    }
}

#[test]
fn find_circle_tangent_towards_clockwise_positive() {
    assert_option_pos(
        find_circle_tangent_towards(
            Position::from_origin_nm(5.0, 6.0),
            Position::from_origin_nm(3.0, 4.0),
            Length::from_nm(2.0),
            TurnDirection::Clockwise,
        ),
        Some(Position::from_origin_nm(3.0, 6.0)),
    );
}

#[test]
fn find_circle_tangent_towards_counter_clockwise_positive() {
    assert_option_pos(
        find_circle_tangent_towards(
            Position::from_origin_nm(5.0, 6.0),
            Position::from_origin_nm(3.0, 4.0),
            Length::from_nm(2.0),
            TurnDirection::CounterClockwise,
        ),
        Some(Position::from_origin_nm(5.0, 4.0)),
    );
}

#[test]
fn find_circle_tangent_towards_clockwise_negative() {
    assert_option_pos(
        find_circle_tangent_towards(
            Position::from_origin_nm(3.0, 4.0),
            Position::from_origin_nm(5.0, 6.0),
            Length::from_nm(2.0),
            TurnDirection::Clockwise,
        ),
        Some(Position::from_origin_nm(5.0, 4.0)),
    );
}

#[test]
fn find_circle_tangent_towards_counter_clockwise_negative() {
    assert_option_pos(
        find_circle_tangent_towards(
            Position::from_origin_nm(3.0, 4.0),
            Position::from_origin_nm(5.0, 6.0),
            Length::from_nm(2.0),
            TurnDirection::CounterClockwise,
        ),
        Some(Position::from_origin_nm(3.0, 6.0)),
    );
}

#[test]
fn find_circle_tangent_towards_inside() {
    assert_option_pos(
        find_circle_tangent_towards(
            Position::from_origin_nm(5.0, 6.0),
            Position::from_origin_nm(5.0, 6.0),
            Length::from_nm(2.0),
            TurnDirection::CounterClockwise,
        ),
        None,
    );
}

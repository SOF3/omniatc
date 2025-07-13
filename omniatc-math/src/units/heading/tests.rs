use bevy_math::Vec2;

use super::{Heading, TurnDirection};
use crate::units::Angle;

fn assert_almost_eq(left: Heading, right: Heading, message: &str) {
    let delta = (left.0 - right.0).abs();
    assert!(
        delta.0 < 1e-4 || (Angle::FULL - delta).abs().0 < 1e-4,
        "{left:?} != {right:?}: {message}"
    );
}

fn assert_delta(left: Angle<f32>, right: Angle<f32>, message: &str) {
    assert!((left - right).abs().0 < 1e-4, "{} != {}: {message}", left.0, right.0);
}

#[test]
fn heading_from_vec2() {
    assert_almost_eq(Heading::from_vec2(Vec2::new(1., 0.)), Heading::EAST, "(1, 0) is eastward");
    assert_almost_eq(Heading::from_vec2(Vec2::new(-1., 0.)), Heading::WEST, "(-1, 0) is westward");
    assert_almost_eq(Heading::from_vec2(Vec2::new(0., 1.)), Heading::NORTH, "(1, 0) is northward");
    assert_almost_eq(
        Heading::from_vec2(Vec2::new(0., -1.)),
        Heading::SOUTH,
        "(-1, 0) is southward",
    );
}

#[test]
fn heading_from_degrees() {
    assert_almost_eq(Heading::from_degrees(-90.), Heading::WEST, "-90 degrees is westward");
    assert_almost_eq(Heading::from_degrees(-270.), Heading::EAST, "-270 degrees is eastward");
    assert_almost_eq(Heading::from_degrees(-360.), Heading::NORTH, "-360 degrees is northward");
    assert_almost_eq(Heading::from_degrees(-180.), Heading::SOUTH, "-180 degrees is southward");
    assert_almost_eq(Heading::from_degrees(90.), Heading::EAST, "90 degrees is eastward");
    assert_almost_eq(Heading::from_degrees(270.), Heading::WEST, "270 degrees is westward");
    assert_almost_eq(Heading::from_degrees(360.), Heading::NORTH, "360 degrees is northward");
    assert_almost_eq(Heading::from_degrees(180.), Heading::SOUTH, "180 degrees is southward");
    assert_almost_eq(Heading::from_degrees(0.), Heading::NORTH, "0 degrees is northward");
}

#[test]
fn heading_distance() {
    assert_delta(
        Heading::WEST.distance(Heading::NORTH, TurnDirection::Clockwise),
        Angle::RIGHT,
        "90 degrees right from west to north",
    );
    assert_delta(
        Heading::WEST.distance(Heading::NORTH, TurnDirection::CounterClockwise),
        Angle::RIGHT * -3.,
        "270 degrees left from west to north",
    );

    assert_delta(
        Heading::EAST.distance(Heading::NORTH, TurnDirection::Clockwise),
        Angle::RIGHT * 3.,
        "270 degrees right from east to north",
    );
    assert_delta(
        Heading::EAST.distance(Heading::NORTH, TurnDirection::CounterClockwise),
        -Angle::RIGHT,
        "90 degrees left from east to north",
    );

    assert_delta(
        Heading::EAST.distance(Heading::WEST, TurnDirection::Clockwise),
        Angle::STRAIGHT,
        "180 degrees from east to west",
    );
    assert_delta(
        Heading::EAST.distance(Heading::WEST, TurnDirection::CounterClockwise),
        -Angle::STRAIGHT,
        "180 degrees from east to west",
    );

    assert_delta(
        Heading::NORTH.distance(Heading::NORTH, TurnDirection::CounterClockwise),
        Angle::ZERO,
        "0 degrees for equal",
    );
    assert_delta(
        Heading::NORTH.distance(Heading::NORTH, TurnDirection::Clockwise),
        Angle::ZERO,
        "0 degrees for equal",
    );
}

#[test]
fn heading_closer_direction() {
    assert_eq!(
        Heading::NORTH.closer_direction_to(Heading::EAST),
        TurnDirection::Clockwise,
        "right turn from north to east"
    );
    assert_eq!(
        Heading::NORTH.closer_direction_to(Heading::WEST),
        TurnDirection::CounterClockwise,
        "left turn from north to east"
    );
    assert_eq!(
        Heading::SOUTH.closer_direction_to(Heading::EAST),
        TurnDirection::CounterClockwise,
        "left turn from north to east"
    );
    assert_eq!(
        Heading::SOUTH.closer_direction_to(Heading::WEST),
        TurnDirection::Clockwise,
        "right turn from north to east"
    );

    assert_eq!(
        Heading::from_degrees(-1.).closer_direction_to(Heading::from_degrees(1.)),
        TurnDirection::Clockwise,
        "right turn positive"
    );
    assert_eq!(
        Heading::from_degrees(1.).closer_direction_to(Heading::from_degrees(-1.)),
        TurnDirection::CounterClockwise,
        "left turn negative"
    );
}

#[test]
fn heading_is_between() {
    assert!(
        Heading::NORTH.is_between(Heading::from_degrees(-1.), Heading::from_degrees(1.)),
        "north is between -1 and 1 degrees"
    );
    assert!(
        !Heading::SOUTH.is_between(Heading::from_degrees(-1.), Heading::from_degrees(1.)),
        "south is not between -1 and 1 degrees"
    );

    assert!(
        Heading::NORTH.is_between(Heading::from_degrees(1.), Heading::from_degrees(-1.)),
        "north is between -1 and 1 degrees"
    );
    assert!(
        !Heading::SOUTH.is_between(Heading::from_degrees(1.), Heading::from_degrees(-1.)),
        "south is not between -1 and 1 degrees"
    );
}

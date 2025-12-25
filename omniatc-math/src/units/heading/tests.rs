use bevy_math::Vec2;

use super::{Heading, TurnDirection};
use crate::units::Angle;

const EPSILON: Angle = Angle::from_radians(1e-4);

fn assert_delta(left: Angle, right: Angle, message: &str) {
    assert!((left - right).abs().0 < 1e-4, "{} != {}: {message}", left.0, right.0);
}

#[test]
fn heading_from_vec2() {
    Heading::from_vec2(Vec2::new(1., 0.))
        .assert_approx(Heading::EAST, EPSILON)
        .expect("(1, 0) is eastward");
    Heading::from_vec2(Vec2::new(-1., 0.))
        .assert_approx(Heading::WEST, EPSILON)
        .expect("(-1, 0) is westward");
    Heading::from_vec2(Vec2::new(0., 1.))
        .assert_approx(Heading::NORTH, EPSILON)
        .expect("(0, 1) is northward");
    Heading::from_vec2(Vec2::new(0., -1.))
        .assert_approx(Heading::SOUTH, EPSILON)
        .expect("(0, -1) is southward");
}

#[test]
fn heading_from_degrees() {
    Heading::from_degrees(-90.)
        .assert_approx(Heading::WEST, EPSILON)
        .expect("-90 degrees is westward");
    Heading::from_degrees(-270.)
        .assert_approx(Heading::EAST, EPSILON)
        .expect("-270 degrees is eastward");
    Heading::from_degrees(-360.)
        .assert_approx(Heading::NORTH, EPSILON)
        .expect("-360 degrees is northward");
    Heading::from_degrees(-180.)
        .assert_approx(Heading::SOUTH, EPSILON)
        .expect("-180 degrees is southward");
    Heading::from_degrees(90.)
        .assert_approx(Heading::EAST, EPSILON)
        .expect("90 degrees is eastward");
    Heading::from_degrees(270.)
        .assert_approx(Heading::WEST, EPSILON)
        .expect("270 degrees is westward");
    Heading::from_degrees(360.)
        .assert_approx(Heading::NORTH, EPSILON)
        .expect("360 degrees is northward");
    Heading::from_degrees(180.)
        .assert_approx(Heading::SOUTH, EPSILON)
        .expect("180 degrees is southward");
    Heading::from_degrees(0.)
        .assert_approx(Heading::NORTH, EPSILON)
        .expect("0 degrees is northward");
}

#[test]
fn heading_distance() {
    Heading::WEST
        .distance(Heading::NORTH, TurnDirection::Clockwise)
        .assert_approx(Angle::RIGHT, EPSILON)
        .expect("90 degrees right from west to north");
    Heading::WEST
        .distance(Heading::NORTH, TurnDirection::CounterClockwise)
        .assert_approx(Angle::RIGHT * -3., EPSILON)
        .expect("270 degrees left from west to north");

    Heading::EAST
        .distance(Heading::NORTH, TurnDirection::Clockwise)
        .assert_approx(Angle::RIGHT * 3., EPSILON)
        .expect("270 degrees right from east to north");
    Heading::EAST
        .distance(Heading::NORTH, TurnDirection::CounterClockwise)
        .assert_approx(-Angle::RIGHT, EPSILON)
        .expect("90 degrees left from east to north");

    Heading::EAST
        .distance(Heading::WEST, TurnDirection::Clockwise)
        .assert_approx(Angle::STRAIGHT, EPSILON)
        .expect("180 degrees from east to west");
    Heading::EAST
        .distance(Heading::WEST, TurnDirection::CounterClockwise)
        .assert_approx(-Angle::STRAIGHT, EPSILON)
        .expect("180 degrees from east to west");

    Heading::NORTH
        .distance(Heading::NORTH, TurnDirection::CounterClockwise)
        .assert_approx(Angle::ZERO, EPSILON)
        .expect("0 degrees for equal");
    Heading::NORTH
        .distance(Heading::NORTH, TurnDirection::Clockwise)
        .assert_approx(Angle::ZERO, EPSILON)
        .expect("0 degrees for equal");
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

#[test]
fn heading_closest_midpoint() {
    Heading::from_degrees(90.0)
        .closest_midpoint(Heading::from_degrees(92.0))
        .assert_approx(Heading::from_degrees(91.0), EPSILON)
        .expect("forward");
    Heading::from_degrees(90.0)
        .closest_midpoint(Heading::from_degrees(88.0))
        .assert_approx(Heading::from_degrees(89.0), EPSILON)
        .expect("backward");
    Heading::from_degrees(-2.0)
        .closest_midpoint(Heading::from_degrees(2.0))
        .assert_approx(Heading::from_degrees(0.0), EPSILON)
        .expect("forward crossing");
    Heading::from_degrees(2.0)
        .closest_midpoint(Heading::from_degrees(-2.0))
        .assert_approx(Heading::from_degrees(0.0), EPSILON)
        .expect("backward crossing");
}

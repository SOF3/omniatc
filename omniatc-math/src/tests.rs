use bevy_math::Vec2;

use super::line_circle_intersect;
use crate::units::{Length, Position, Squared, TurnDirection};
use crate::{find_circle_tangent_towards, range_steps};

#[test]
fn range_steps_exact() {
    assert_eq!(range_steps(0.0, 2.0, 0.5).collect::<Vec<_>>(), vec![0.0, 0.5, 1.0, 1.5, 2.0]);
}

#[test]
fn range_steps_excess() {
    assert_eq!(range_steps(0.0, 2.3, 0.5).collect::<Vec<_>>(), vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.3]);
}

#[test]
fn range_steps_singleton() {
    assert_eq!(range_steps(0.3, 0.3, 0.5).collect::<Vec<_>>(), vec![0.3]);
}

#[test]
fn range_steps_empty() {
    assert_eq!(range_steps(0.3, 0.2, 0.5).collect::<Vec<_>>(), Vec::<f32>::new());
}

use bevy::ecs::world::EntityRef;
use bevy::math::Vec2;

use crate::level::{nav, object};
use crate::units::{Position, Speed};

pub struct State {
    pub ground_position: Position<Vec2>,
    pub airspeed:        Speed<f32>,
    pub altitude:        Position<f32>,
}

pub fn current_state(entity: EntityRef) -> Option<State> {
    let object = entity.get::<object::Object>()?;
    let vel_target = entity.get::<nav::VelocityTarget>()?;
    Some(State {
        ground_position: object.position.horizontal(),
        airspeed:        vel_target.horiz_speed,
        altitude:        object.position.altitude(),
    })
}

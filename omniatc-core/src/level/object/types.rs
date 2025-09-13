use bevy::ecs::component::Component;
use math::Length;

use crate::level::{nav, taxi};

#[derive(Component)]
pub enum Type {
    Plane { taxi: taxi::Limits, nav: nav::Limits },
}

impl Type {
    #[must_use]
    pub fn half_length(&self) -> Length<f32> {
        match self {
            Type::Plane { taxi, .. } => taxi.half_length,
        }
    }
}

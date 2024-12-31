//! A wind entity applies a velocity component to objects in its effective region.

use bevy::ecs::system::SystemParam;
use bevy::math::bounding::Aabb3d;
use bevy::math::{Vec2, Vec3A};
use bevy::prelude::{Bundle, Component, Entity, EntityCommand, Query, With, World};

/// The direction and strength of wind.
#[derive(Component)]
pub struct Vector {
    /// The wind vector at the lowest altitude of the region.
    pub bottom: Vec2,
    /// The wind vector at the highest altitude of the region.
    pub top:    Vec2,
}

/// This wind entity only applies to objects in the AABB.
#[derive(Component)]
pub struct EffectRegion(pub Aabb3d);

/// Marker component for wind entities.
#[derive(Component)]
pub struct Marker;

#[derive(Bundle)]
pub struct Comps {
    pub vector:        Vector,
    pub effect_region: EffectRegion,
}

pub struct SpawnCommand {
    pub bundle: Comps,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity).insert((self.bundle, Marker));
    }
}

/// Locates the winds effective at a point.
#[derive(SystemParam)]
pub struct Locator<'w, 's> {
    wind_query: Query<'w, 's, (&'static Vector, &'static EffectRegion), With<Marker>>,
}

impl Locator<'_, '_> {
    /// Computes the total wind component at a point.
    pub fn locate(&self, object: impl Into<Vec3A>) -> Vec2 {
        // TODO use an appropriate range query data structure if necessary.
        let object = object.into();
        self.wind_query
            .iter()
            .filter_map(|(vector, EffectRegion(region))| {
                if (region.min.cmple(object) & region.max.cmpge(object)).all() {
                    let level = (object.z - region.min.z) / (region.max.z - region.min.z);
                    Some(vector.bottom.lerp(vector.top, level))
                } else {
                    None
                }
            })
            .sum()
    }
}

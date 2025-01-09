//! A wind entity applies a velocity component to objects in its effective region.

use bevy::ecs::system::SystemParam;
use bevy::math::bounding::Aabb3d;
use bevy::math::{Vec2, Vec3A};
use bevy::prelude::{Bundle, Component, Entity, EntityCommand, Query, With, World};

use crate::units::{Position, Speed};

/// The direction and strength of wind.
#[derive(Component)]
pub struct Vector {
    /// The wind vector at the lowest altitude of the region.
    pub bottom: Speed<Vec2>,
    /// The wind vector at the highest altitude of the region.
    pub top:    Speed<Vec2>,
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
    pub fn locate(&self, object_pos: Position<impl Into<Vec3A>>) -> Speed<Vec2> {
        // TODO use an appropriate range query data structure if necessary.
        let object_pos = object_pos.get().into();
        self.wind_query
            .iter()
            .filter_map(|(vector, EffectRegion(region))| {
                if (region.min.cmple(object_pos) & region.max.cmpge(object_pos)).all() {
                    let level = (object_pos.z - region.min.z) / (region.max.z - region.min.z);
                    Some(vector.bottom.lerp(vector.top, level))
                } else {
                    None
                }
            })
            .sum()
    }
}

use bevy::ecs::name::Name;
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::World;
use bevy::math::bounding::Aabb3d;

use crate::level::wind;
use crate::load::StoredEntity;

pub fn spawn(world: &mut World, winds: &[store::Wind]) {
    for wind in winds {
        let entity = world.spawn((StoredEntity, Name::new("Wind"))).id();
        wind::SpawnCommand {
            bundle: wind::Comps {
                vector:        wind::Vector { bottom: wind.bottom_speed, top: wind.top_speed },
                effect_region: wind::EffectRegion(Aabb3d {
                    min: wind.start.with_altitude(wind.bottom).get().into(),
                    max: wind.end.with_altitude(wind.top).get().into(),
                }),
            },
        }
        .apply(world.entity_mut(entity));
    }
}

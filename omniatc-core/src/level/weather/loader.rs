use bevy::ecs::name::Name;
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::World;
use bevy::math::bounding::Aabb2d;

use crate::level::weather;
use crate::load::StoredEntity;

pub fn spawn(world: &mut World, weathers: &[store::Weather]) {
    for weather in weathers {
        let entity = world.spawn((StoredEntity, Name::new("Weather"))).id();
        weather::SpawnCommand {
            bundle: weather::Comps {
                weather:       weather::Weather {
                    sea_pressure:         weather.sea_pressure,
                    sea_temp:             weather.sea_temp,
                    sea_wind:             weather.sea_wind,
                    wind_scaling_per_nm:  weather.wind_scaling_per_nm,
                    wind_rotation_per_nm: weather.wind_rotation_per_nm,
                },
                effect_region: weather::EffectRegion(Aabb2d {
                    min: weather.start.get(),
                    max: weather.end.get(),
                }),
            },
        }
        .apply(world.entity_mut(entity));
    }
}

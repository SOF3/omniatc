use std::collections::HashMap;

use bevy::ecs::entity::Entity;
use bevy::ecs::name::Name;
use bevy::ecs::relationship::{RelatedSpawner, Relationship};
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::World;
use math::{Angle, Length, SEA_ALTITUDE};

use crate::level::aerodrome::loader::AerodromeMap;
use crate::level::navaid::{self, Navaid};
use crate::level::waypoint::{self, Waypoint};
use crate::load::{self, LoadedEntity};

/// Spawns named waypoints declared in a store into the world.
pub fn spawn(world: &mut World, waypoints: &[store::Waypoint]) -> WaypointMap {
    WaypointMap(
        waypoints
            .iter()
            .map(|waypoint| {
                let waypoint_entity = world
                    .spawn((LoadedEntity, Name::new(format!("Waypoint: {}", waypoint.name))))
                    .id();
                waypoint::SpawnCommand {
                    waypoint: Waypoint {
                        name:         waypoint.name.clone(),
                        display_type: choose_waypoint_display_type(&waypoint.navaids),
                        position:     waypoint
                            .position
                            .with_altitude(waypoint.elevation.unwrap_or(SEA_ALTITUDE)),
                    },
                }
                .apply(world.entity_mut(waypoint_entity));

                world.entity_mut(waypoint_entity).with_related_entities::<navaid::OwnerWaypoint>(
                    |b| {
                        waypoint.navaids.iter().for_each(|navaid| spawn_waypoint_navaid(b, navaid));
                    },
                );

                (waypoint.name.clone(), waypoint_entity)
            })
            .collect::<HashMap<_, _>>(),
    )
}

fn choose_waypoint_display_type(navaids: &[store::Navaid]) -> waypoint::DisplayType {
    let has_vor = navaids.iter().any(|navaid| matches!(navaid.ty, store::NavaidType::Vor));
    let has_dme = navaids.iter().any(|navaid| matches!(navaid.ty, store::NavaidType::Dme));
    if has_vor && has_dme {
        waypoint::DisplayType::VorDme
    } else if has_vor {
        waypoint::DisplayType::Vor
    } else if has_dme {
        waypoint::DisplayType::Dme
    } else {
        waypoint::DisplayType::Waypoint
    }
}

fn spawn_waypoint_navaid(b: &mut RelatedSpawner<'_, impl Relationship>, navaid: &store::Navaid) {
    b.spawn((Navaid {
        kind:                match navaid.ty {
            store::NavaidType::Vor => navaid::Kind::Vor,
            store::NavaidType::Dme => navaid::Kind::Dme,
        },
        heading_range:       navaid.heading_start..navaid.heading_end,
        pitch_range_tan:     navaid.min_pitch.acute_signed_tan()..Angle::RIGHT.acute_signed_tan(),
        min_dist_horizontal: Length::ZERO,
        min_dist_vertical:   Length::ZERO,
        max_dist_horizontal: navaid.max_dist_horizontal,
        max_dist_vertical:   navaid.max_dist_vertical,
    },));
}

pub struct WaypointMap(HashMap<String, Entity>);

impl WaypointMap {
    /// Resolve a named waypoint reference.
    ///
    /// # Errors
    /// If the referenced waypoint or runway does not exist.
    pub fn resolve(&self, name: &str) -> Result<Entity, load::Error> {
        self.0.get(name).copied().ok_or_else(|| load::Error::UnresolvedWaypoint(name.to_string()))
    }

    /// Resolve a waypoint reference, including runway virtual waypoints.
    ///
    /// # Errors
    /// If the referenced waypoint or runway does not exist.
    pub fn resolve_ref(
        &self,
        aerodromes: &AerodromeMap,
        waypoint_ref: &store::WaypointRef,
    ) -> Result<Entity, load::Error> {
        match waypoint_ref {
            store::WaypointRef::Named(name) => self.resolve(name),
            store::WaypointRef::RunwayThreshold(runway_ref) => {
                Ok(aerodromes.resolve_runway_ref(runway_ref)?.runway.runway)
            }
            store::WaypointRef::LocalizerStart(runway_ref) => {
                Ok(aerodromes.resolve_runway_ref(runway_ref)?.runway.localizer_waypoint)
            }
        }
    }
}

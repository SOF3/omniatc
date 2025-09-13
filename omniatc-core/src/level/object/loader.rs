use std::collections::HashMap;

use bevy::ecs::entity::Entity;
use bevy::ecs::name::Name;
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::{EntityWorldMut, World};
use math::Speed;
use store::YawTarget;

use crate::level::aerodrome::loader::AerodromeMap;
use crate::level::dest::Destination;
use crate::level::route::loader::RoutePresetMap;
use crate::level::route::{self, Route};
use crate::level::waypoint::loader::WaypointMap;
use crate::level::{nav, object, plane, taxi, wake};
use crate::load::{self, StoredEntity};

/// Spawns object types declared in a store into the world.
pub fn spawn_types(
    world: &mut World,
    types: &HashMap<store::ObjectTypeRef, store::ObjectType>,
) -> ObjectTypeMap {
    let mut out = HashMap::new();
    for (ref_id, ty) in types {
        match &ty.class {
            store::ObjectClassSpec::Plane { nav_limits } => {
                let id = world
                    .spawn((
                        StoredEntity,
                        Name::new(format!("Type: {}", ty.full_name)),
                        object::types::Type::Plane {
                            taxi: taxi::Limits(ty.taxi_limits.clone()),
                            nav:  nav::Limits(nav_limits.clone()),
                        },
                    ))
                    .id();
                out.insert(ref_id.clone(), id);
            }
        }
    }
    ObjectTypeMap(out)
}

pub struct ObjectTypeMap(HashMap<store::ObjectTypeRef, Entity>);

impl ObjectTypeMap {
    /// Resolves an object type by reference.
    ///
    /// # Errors
    /// If the object type reference does not exist.
    pub fn resolve(&self, r: &store::ObjectTypeRef) -> load::Result<Entity> {
        match self.0.get(r) {
            Some(&entity) => Ok(entity),
            None => Err(load::Error::UnresolvedObjectType(r.0.clone())),
        }
    }
}

/// Spawns objects declared in a store into the world.
///
/// # Errors
/// If the stored objects contain invalid references.
pub fn spawn(
    world: &mut World,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    route_presets: &RoutePresetMap,
    objects: &[store::Object],
) -> Result<(), load::Error> {
    for object in objects {
        match object {
            store::Object::Plane(plane) => {
                spawn_plane(world, aerodromes, waypoints, route_presets, plane)?;
            }
        }
    }

    Ok(())
}

fn spawn_plane(
    world: &mut World,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    route_presets: &RoutePresetMap,
    plane: &store::Plane,
) -> Result<(), load::Error> {
    let plane_entity =
        world.spawn((StoredEntity, Name::new(format!("Plane: {}", plane.aircraft.name)))).id();

    let destination = resolve_destination(aerodromes, waypoints, &plane.aircraft.dest)?;

    object::SpawnCommand {
        position: plane.aircraft.position.with_altitude(plane.aircraft.altitude),
        ground_speed: (plane.aircraft.ground_speed * plane.aircraft.ground_dir)
            .with_vertical(plane.aircraft.vert_rate),
        display: object::Display { name: plane.aircraft.name.clone() },
        destination,
        completion_score: Some(plane.aircraft.completion_score),
    }
    .apply(world.entity_mut(plane_entity));

    world.entity_mut(plane_entity).insert(taxi::Limits(plane.taxi_limits.clone()));

    plane::SpawnCommand {
        control: Some(plane::Control {
            heading:     plane.control.heading,
            yaw_speed:   plane.control.yaw_speed,
            horiz_accel: plane.control.horiz_accel,
        }),
        limits:  nav::Limits(plane.nav_limits.clone()),
    }
    .apply(world.entity_mut(plane_entity));

    match &plane.nav_target {
        store::NavTarget::Airborne(target) => {
            object::SetAirborneCommand.apply(world.entity_mut(plane_entity));

            let mut plane_ref = world.entity_mut(plane_entity);
            let airspeed = plane_ref
                .get::<object::Airborne>()
                .expect("inserted by SetAirborneCommand")
                .airspeed;

            let dt_target = nav::VelocityTarget {
                yaw:         YawTarget::Heading(airspeed.horizontal().heading()),
                horiz_speed: airspeed.horizontal().magnitude_exact(),
                vert_rate:   Speed::ZERO,
                expedite:    false,
            };

            plane_ref.insert(dt_target);

            insert_airborne_nav_targets(&mut plane_ref, aerodromes, waypoints, target)?;
        }
        store::NavTarget::Ground(target) => {
            let (segment, segment_direction) = aerodromes.resolve_closest_segment_by_label(
                &target.segment,
                plane.aircraft.position,
                plane.aircraft.ground_dir,
                &plane.aircraft.name,
            )?;

            object::SetOnGroundCommand {
                segment,
                direction: segment_direction,
                heading: Some(plane.aircraft.ground_dir),
            }
            .apply(world.entity_mut(plane_entity));
        }
    }

    world.entity_mut(plane_entity).insert((
        route::Id(plane.route.id.clone()),
        route::loader::convert_route(aerodromes, waypoints, route_presets, &plane.route.nodes)
            .collect::<load::Result<Route>>()?,
    ));
    route::RunCurrentNode.apply(world.entity_mut(plane_entity));

    insert_wake(world.entity_mut(plane_entity), plane);

    Ok(())
}

/// Resolves a stored destination into a runtime destination.
///
/// # Errors
/// If the stored destination references invalid aerodromes or waypoints.
pub fn resolve_destination(
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    dest: &store::Destination,
) -> load::Result<Destination> {
    Ok(match *dest {
        store::Destination::Landing { ref aerodrome } => {
            let aerodrome = aerodromes.resolve(aerodrome)?;
            Destination::Landing { aerodrome: aerodrome.aerodrome_entity }
        }
        store::Destination::Parking { ref aerodrome } => {
            let aerodrome = aerodromes.resolve(aerodrome)?;
            Destination::Parking { aerodrome: aerodrome.aerodrome_entity }
        }
        store::Destination::VacateAnyRunway => Destination::VacateAnyRunway,
        store::Destination::Departure { min_altitude, ref waypoint_proximity } => {
            let waypoint_proximity = waypoint_proximity
                .as_ref()
                .map(|&(ref waypoint, dist)| {
                    let waypoint = waypoints.resolve_ref(aerodromes, waypoint)?;
                    Ok((waypoint, dist))
                })
                .transpose()?;
            Destination::Departure { min_altitude, waypoint_proximity }
        }
    })
}

fn insert_airborne_nav_targets(
    plane_entity: &mut EntityWorldMut,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    target: &store::AirborneNavTarget,
) -> Result<(), load::Error> {
    if let Some(target_altitude) = &target.target_altitude {
        plane_entity.insert(nav::TargetAltitude {
            altitude: target_altitude.altitude,
            expedite: target_altitude.expedite,
        });
    }

    if let Some(target_glide) = &target.target_glide {
        let target_waypoint = waypoints.resolve_ref(aerodromes, &target_glide.target_waypoint)?;
        plane_entity.insert(nav::TargetGlide {
            target_waypoint,
            glide_angle: target_glide.glide_angle,
            min_pitch: target_glide.min_pitch,
            max_pitch: target_glide.max_pitch,
            lookahead: target_glide.lookahead,
            expedite: target_glide.expedite,
        });
    }

    if let Some(target_waypoint) = &target.target_waypoint {
        let waypoint_entity = waypoints.resolve_ref(aerodromes, &target_waypoint.waypoint)?;
        plane_entity.insert(nav::TargetWaypoint { waypoint_entity });
    }

    if let Some(target_alignment) = &target.target_alignment {
        let start_waypoint = waypoints.resolve_ref(aerodromes, &target_alignment.start_waypoint)?;
        let end_waypoint = waypoints.resolve_ref(aerodromes, &target_alignment.end_waypoint)?;
        plane_entity.insert(nav::TargetAlignment {
            start_waypoint,
            end_waypoint,
            lookahead: target_alignment.lookahead,
            activation_range: target_alignment.activation_range,
        });
    }

    Ok(())
}

const WAKE_FACTOR: f32 = 10.;

fn insert_wake(mut plane_entity: EntityWorldMut, plane: &store::Plane) {
    plane_entity.insert((
        wake::Producer { base_intensity: compute_wake(&plane.taxi_limits, &plane.nav_limits) },
        wake::Detector::default(),
    ));
}

/// Computes the wake turbulence intensity for a plane based on its taxi and navigation limits.
#[must_use]
pub fn compute_wake(
    taxi_limits: &store::TaxiLimits,
    nav_limits: &store::NavLimits,
) -> wake::Intensity {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // nearest positive integer
    wake::Intensity((WAKE_FACTOR * nav_limits.weight / taxi_limits.width.into_nm()) as u32)
}

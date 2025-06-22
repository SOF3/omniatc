use std::borrow::Cow;
use std::collections::HashMap;
use std::f32::consts;
use std::{io, iter};

use bevy::ecs::relationship::{RelatedSpawner, Relationship};
use bevy::math::bounding::Aabb3d;
use bevy::math::Vec2;
use bevy::prelude::{
    Command as BevyCommand, Entity, EntityCommand, EntityWorldMut, Name, With, World,
};
use either::Either;
use itertools::Itertools;

use crate::level::navaid::{self, Navaid};
use crate::level::route::{self, Route};
use crate::level::runway::Runway;
use crate::level::waypoint::{self, Waypoint};
use crate::level::{aerodrome, ground, nav, object, plane, runway, wake, wind};
use crate::math::sweep::LineSweeper;
use crate::math::{sweep, SEA_ALTITUDE};
use crate::store;
use crate::units::{Angle, Distance, Heading, Position, Speed};

#[cfg(test)]
mod tests;

const WAKE_FACTOR: f32 = 10.;

pub enum Source {
    Raw(Cow<'static, [u8]>),
    Parsed(Box<store::File>),
}

pub struct Command {
    pub source:   Source,
    pub on_error: Box<dyn FnOnce(&mut World, Error) + Send>,
}

impl BevyCommand for Command {
    fn apply(self, world: &mut World) {
        if let Err(err) = do_load(world, &self.source) {
            (self.on_error)(world, err);
        }
    }
}

fn do_load(world: &mut World, source: &Source) -> Result<(), Error> {
    let file_owned: store::File;
    let file = match source {
        Source::Raw(bytes) => {
            file_owned = ciborium::from_reader(bytes.as_ref()).map_err(Error::Serde)?;
            &file_owned
        }
        Source::Parsed(file) => file,
    };

    world
        .query_filtered::<Entity, With<store::LoadedEntity>>()
        .iter(world)
        .collect::<Vec<_>>()
        .into_iter()
        .for_each(|entity| world.entity_mut(entity).despawn());

    spawn_winds(world, &file.level.environment.winds);
    let aerodromes = spawn_aerodromes(world, &file.level.aerodromes)?;
    let waypoints = spawn_waypoints(world, &file.level.waypoints)?;

    let route_presets =
        spawn_route_presets(world, &aerodromes, &waypoints, &file.level.route_presets)?;

    spawn_objects(world, &aerodromes, &waypoints, &route_presets, &file.level.objects)?;

    world.resource_mut::<store::CameraAdvice>().0 = Some(file.ui.camera.clone());

    Ok(())
}

fn spawn_winds(world: &mut World, winds: &[store::Wind]) {
    for wind in winds {
        let entity = world.spawn((store::LoadedEntity, Name::new("Wind"))).id();
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

fn spawn_aerodromes(
    world: &mut World,
    aerodromes: &[store::Aerodrome],
) -> Result<AerodromeMap, Error> {
    aerodromes
        .iter()
        .enumerate()
        .map(|(id, aerodrome)| {
            let aerodrome_id = u32::try_from(id).map_err(|_| Error::TooManyAerodromes)?;
            let aerodrome_entity = world
                .spawn((
                    store::LoadedEntity,
                    Name::new(format!("Aerodrome: {}", aerodrome.code)),
                    aerodrome::Aerodrome {
                        id:   aerodrome_id,
                        code: aerodrome.code.clone(),
                        name: aerodrome.full_name.clone(),
                    },
                ))
                .id();
            world.send_event(aerodrome::SpawnEvent(aerodrome_entity));

            let mut runway_entities = HashMap::new();
            for runway_pair in &aerodrome.runways {
                runway_entities.insert(
                    runway_pair.forward.name.clone(),
                    spawn_runway(
                        world,
                        aerodrome,
                        runway_pair.width,
                        &runway_pair.forward,
                        runway_pair.forward_start,
                        runway_pair.backward_start,
                        aerodrome_entity,
                    ),
                );
                runway_entities.insert(
                    runway_pair.backward.name.clone(),
                    spawn_runway(
                        world,
                        aerodrome,
                        runway_pair.width,
                        &runway_pair.backward,
                        runway_pair.backward_start,
                        runway_pair.forward_start,
                        aerodrome_entity,
                    ),
                );
            }

            spawn_ground_segments(
                world,
                &aerodrome.ground_network,
                &aerodrome.runways,
                &runway_entities,
                aerodrome_entity,
                aerodrome.elevation,
            )?;

            Ok((
                aerodrome.code.clone(),
                SpawnedAerodrome {
                    aerodrome_entity,
                    index: aerodrome_id,
                    runways: runway_entities,
                },
            ))
        })
        .collect::<HashMapResult<_, _>>()
        .map(AerodromeMap)
}

struct AerodromeMap(HashMap<String, SpawnedAerodrome>);

impl AerodromeMap {
    fn resolve(&self, code: &str) -> Result<&SpawnedAerodrome, Error> {
        self.0.get(code).ok_or_else(|| Error::UnresolvedAerodrome(code.to_string()))
    }
}

struct SpawnedAerodrome {
    aerodrome_entity: Entity,
    index:            u32,
    runways:          HashMap<String, SpawnedRunway>,
}

struct SpawnedRunway {
    runway:             Entity,
    localizer_waypoint: Entity,
}

fn spawn_runway(
    world: &mut World,
    aerodrome: &store::Aerodrome,
    runway_width: Distance<f32>,
    runway: &store::Runway,
    start_pos: Position<Vec2>,
    end_pos: Position<Vec2>,
    aerodrome_entity: Entity,
) -> SpawnedRunway {
    let runway_entity = world
        .spawn((
            store::LoadedEntity,
            Name::new(format!("Runway: {}/{}", aerodrome.code, runway.name)),
        ))
        .id();

    let heading = Heading::from_vec2((end_pos - start_pos).0);
    let touchdown_position =
        (start_pos + runway.touchdown_displacement * heading).with_altitude(aerodrome.elevation);

    runway::SpawnCommand {
        waypoint: Waypoint {
            name:         runway.name.clone(),
            display_type: waypoint::DisplayType::Runway,
            position:     touchdown_position,
        },
        runway:   Runway {
            aerodrome:      aerodrome_entity,
            landing_length: (end_pos - start_pos).normalize_to_magnitude(
                start_pos.distance_exact(end_pos) - runway.touchdown_displacement,
            ),
            glide_descent:  runway.glide_angle,
            display_start:  start_pos.with_altitude(aerodrome.elevation),
            display_end:    end_pos.with_altitude(aerodrome.elevation),
            width:          runway_width,
        },
    }
    .apply(world.entity_mut(runway_entity));

    world.entity_mut(runway_entity).with_related_entities::<navaid::OwnerWaypoint>(|b| {
        spawn_runway_navaids(b, heading, runway);
    });

    let localizer_waypoint = world
        .spawn((
            store::LoadedEntity,
            Name::new(format!("LocalizerWaypoint: {}/{}", aerodrome.code, runway.name)),
            runway::LocalizerWaypoint { runway_ref: runway_entity },
        ))
        .id();
    waypoint::SpawnCommand {
        waypoint: Waypoint {
            name:         format!("ILS:{}/{}", aerodrome.code, runway.name),
            display_type: waypoint::DisplayType::None,
            position:     touchdown_position
                + (runway.max_visual_distance * heading.opposite())
                    .projected_from_elevation_angle(runway.glide_angle),
        },
    }
    .apply(world.entity_mut(localizer_waypoint));

    world.entity_mut(runway_entity).insert(runway::LocalizerWaypointRef { localizer_waypoint });

    SpawnedRunway { runway: runway_entity, localizer_waypoint }
}

fn spawn_runway_navaids(
    b: &mut RelatedSpawner<'_, impl Relationship>,
    heading: Heading,
    runway: &store::Runway,
) {
    b.spawn((
        Navaid {
            kind:                navaid::Kind::Visual,
            heading_range:       Heading::NORTH..Heading::NORTH,
            pitch_range_tan:     Angle::ZERO.acute_signed_tan()..Angle::RIGHT.acute_signed_tan(),
            min_dist_horizontal: Distance::ZERO,
            min_dist_vertical:   Distance::ZERO,
            // TODO overwrite these two fields with visibility
            max_dist_horizontal: runway.max_visual_distance,
            max_dist_vertical:   Distance(100.),
        },
        navaid::Visual { max_range: runway.max_visual_distance },
    ));

    if let Some(ils) = &runway.ils {
        b.spawn((
            Navaid {
                kind:                navaid::Kind::Localizer,
                heading_range:       (heading.opposite() - ils.half_width)
                    ..(heading.opposite() + ils.half_width),
                pitch_range_tan:     ils.min_pitch.acute_signed_tan()
                    ..ils.max_pitch.acute_signed_tan(),
                min_dist_horizontal: ils.visual_range,
                min_dist_vertical:   ils.decision_height,
                max_dist_horizontal: ils.horizontal_range,
                max_dist_vertical:   ils.vertical_range,
            },
            navaid::LandingAid,
        ));
    }
}

const GROUND_EPSILON: Distance<f32> = Distance::from_meters(1.);

fn collect_non_apron_ground_lines(
    ground_network: &store::GroundNetwork,
    runway_pairs: &[store::RunwayPair],
    runways: &HashMap<String, SpawnedRunway>,
) -> Vec<GroundLine> {
    runway_pairs
        .iter()
        .map(|pair| {
            let forward_runway = runways
                .get(&pair.forward.name)
                .expect("all declared runways have been inserted into `runways`");
            let backward_runway = runways
                .get(&pair.backward.name)
                .expect("all declared runways have been inserted into `runways`");
            GroundLine {
                label: ground::SegmentLabel::RunwayPair([
                    forward_runway.runway,
                    backward_runway.runway,
                ]),
                width: pair.width,
                alpha: pair.forward_start
                    + (pair.forward_start - pair.backward_start)
                        .normalize_to_magnitude(pair.backward.stopway),
                beta:  pair.backward_start
                    + (pair.backward_start - pair.forward_start)
                        .normalize_to_magnitude(pair.forward.stopway),
            }
        })
        .chain(ground_network.taxiways.iter().flat_map(|taxiway| {
            taxiway.endpoints.iter().tuple_windows().map(|(&alpha, &beta)| GroundLine {
                label: ground::SegmentLabel::Taxiway { name: taxiway.name.clone() },
                width: taxiway.width,
                alpha,
                beta,
            })
        }))
        .collect()
}

fn generate_apron_lines(
    ground_network: &store::GroundNetwork,
    lines: &mut Vec<GroundLine>,
) -> Result<(), Error> {
    let mut aprons: Vec<_> = ground_network
        .aprons
        .iter()
        .map(|apron| {
            let heading = apron
                .forward_heading
                .opposite()
                .as_ordered()
                .map_err(|_| Error::NonFiniteFloat("apron forward_heading"))?;
            Ok((heading, apron))
        })
        .collect::<Result<_, Error>>()?;
    aprons.sort_by_key(|apron| apron.0);

    let non_apron_lines_len = lines.len();
    lines.extend(aprons.iter().map(|(_, apron)| GroundLine {
        label: ground::SegmentLabel::Apron { name: apron.name.clone() },
        width: apron.width,
        alpha: apron.position,
        beta:  apron.position, // we will update this later
    }));
    let (non_apron_lines, apron_lines) = lines.split_at_mut(non_apron_lines_len);

    for (apron_index, &(_apron_back, apron)) in aprons.iter().enumerate() {
        let sweeper = LineSweeper::new(
            |index| match index.0.checked_sub(1) {
                None => sweep::Line {
                    alpha:          apron.position,
                    beta:           apron.position
                        - Distance::from_nm(100.) * apron.forward_heading,
                    need_intersect: true,
                },
                Some(non_apron_index) => {
                    let line = &non_apron_lines[non_apron_index];
                    sweep::Line {
                        alpha:          line.alpha,
                        beta:           line.beta,
                        need_intersect: false,
                    }
                }
            },
            non_apron_lines.len() + 1,
            GROUND_EPSILON,
            apron.forward_heading.opposite().into_dir2(),
        )
        .map_err(Error::GroundSweep)?;

        let intersect = sweeper
            .intersections_after(apron.position)
            .next()
            .ok_or_else(|| Error::UnreachableApron(apron.name.clone()))?;
        // previously inited with a dummy value
        apron_lines[apron_index].beta = intersect.position;
    }

    Ok(())
}

struct IntersectGroup {
    index:    usize,
    position: Position<Vec2>,
    lines:    Vec<sweep::LineIndex>,
}

fn find_ground_intersects(lines: &[GroundLine]) -> Result<Vec<IntersectGroup>, Error> {
    let intersects: Vec<_> = LineSweeper::new(
        |index| {
            let line = &lines[index.0];
            sweep::Line {
                alpha:          line.alpha,
                beta:           line.beta,
                need_intersect: true,
            }
        },
        lines.len(),
        Distance::from_meters(0.1),
        Heading::from_radians(Angle(consts::E)).into_dir2(), // an arbitrary direction to avoid duplicates
    )
    .map_err(Error::GroundSweep)?
    .intersections_merged()
    .enumerate()
    .map(|(group_index, group)| {
        let mut position = group[0].position;
        #[expect(clippy::cast_precision_loss)] // `i` is expected to be small
        for (i, intersect) in group[1..].iter().enumerate() {
            position = position.lerp(intersect.position, 1. / (i + 1) as f32);
        }

        let mut lines = Vec::new();
        for intersect in &group {
            for line in intersect.lines {
                if !lines.contains(&line) {
                    lines.push(line);
                }
            }
        }

        IntersectGroup { index: group_index, position, lines }
    })
    .collect();
    Ok(intersects)
}

struct GroundSegment {
    alpha: GroundSegmentEndpoint,
    beta:  GroundSegmentEndpoint,
    line:  sweep::LineIndex,
}

#[derive(Clone)]
struct GroundSegmentEndpoint {
    position: Position<Vec2>,
    group:    Option<usize>,
}

fn ground_lines_to_segments(
    lines: &[GroundLine],
    all_intersect_groups: &[IntersectGroup],
) -> Result<Vec<GroundSegment>, Error> {
    let mut line_to_intersects_map = HashMap::<_, Vec<_>>::new();
    for group in all_intersect_groups {
        for line in &group.lines {
            let alpha_dist = group
                .position
                .distance_ord(lines[line.0].alpha)
                .map_err(|_| Error::NonFiniteFloat("evaluated intersection point"))?;
            line_to_intersects_map.entry(line).or_default().push((alpha_dist, group));
        }
    }

    let mut segments = Vec::new();

    for (line_index, line) in lines.iter().enumerate() {
        let line_index = sweep::LineIndex(line_index);
        let intersects = line_to_intersects_map
            .get_mut(&line_index)
            .map_or_else(Default::default, |vec| &mut vec[..]);
        intersects.sort_by_key(|&(alpha_dist, _)| alpha_dist);

        let alpha_endpoint = intersects
            .first()
            .is_none_or(|&(_, group)| group.position.distance_cmp(line.alpha) > GROUND_EPSILON)
            .then_some(GroundSegmentEndpoint { position: line.alpha, group: None });
        let beta_endpoint = intersects
            .last()
            .is_none_or(|&(_, group)| group.position.distance_cmp(line.beta) > GROUND_EPSILON)
            .then_some(GroundSegmentEndpoint { position: line.beta, group: None });

        segments.extend(
            alpha_endpoint
                .into_iter()
                .chain(intersects.iter().map(|&(_, group)| GroundSegmentEndpoint {
                    position: group.position,
                    group:    Some(group.index),
                }))
                .chain(beta_endpoint)
                .tuple_windows()
                .map(|(alpha, beta)| GroundSegment { alpha, beta, line: line_index }),
        );
    }

    Ok(segments)
}

fn spawn_ground_segments(
    world: &mut World,
    ground_network: &store::GroundNetwork,
    runway_pairs: &[store::RunwayPair],
    runways: &HashMap<String, SpawnedRunway>,
    aerodrome_entity: Entity,
    elevation: Position<f32>,
) -> Result<(), Error> {
    let mut lines = collect_non_apron_ground_lines(ground_network, runway_pairs, runways);
    generate_apron_lines(ground_network, &mut lines)?;
    let intersect_groups = find_ground_intersects(&lines)?;
    let segments = ground_lines_to_segments(&lines, &intersect_groups)?;

    let endpoints: Vec<_> = intersect_groups
        .iter()
        .map(|group| {
            let entity = world.spawn_empty().insert(ground::EndpointOf(aerodrome_entity)).id();
            ground::SpawnEndpoint { position: group.position }.apply(world.entity_mut(entity));
            entity
        })
        .collect();

    for segment in segments {
        let [alpha_endpoint, beta_endpoint] = [&segment.alpha, &segment.beta].map(|endpoint| {
            if let Some(index) = endpoint.group {
                endpoints[index]
            } else {
                // spawn a non-intersecting endpoint only used in this segment
                let entity = world.spawn_empty().insert(ground::EndpointOf(aerodrome_entity)).id();
                ground::SpawnEndpoint { position: endpoint.position }
                    .apply(world.entity_mut(entity));
                entity
            }
        });

        let segment_entity = world.spawn_empty().insert(ground::SegmentOf(aerodrome_entity)).id();
        let &GroundLine { ref label, width, .. } = &lines[segment.line.0];
        ground::SpawnSegment {
            segment: ground::Segment {
                alpha: alpha_endpoint,
                beta: beta_endpoint,
                width,
                elevation,
            },
            label:   label.clone(),
        }
        .apply(world.entity_mut(segment_entity));
    }

    Ok(())
}

struct GroundLine {
    label: ground::SegmentLabel,
    width: Distance<f32>,
    alpha: Position<Vec2>,
    beta:  Position<Vec2>,
}

fn spawn_waypoints(world: &mut World, waypoints: &[store::Waypoint]) -> Result<WaypointMap, Error> {
    waypoints
        .iter()
        .map(|waypoint| {
            let waypoint_entity = world
                .spawn((store::LoadedEntity, Name::new(format!("Waypoint: {}", waypoint.name))))
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

            world.entity_mut(waypoint_entity).with_related_entities::<navaid::OwnerWaypoint>(|b| {
                waypoint.navaids.iter().for_each(|navaid| spawn_waypoint_navaid(b, navaid));
            });

            Ok((waypoint.name.clone(), waypoint_entity))
        })
        .collect::<HashMapResult<_, _>>()
        .map(WaypointMap)
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
        min_dist_horizontal: Distance::ZERO,
        min_dist_vertical:   Distance::ZERO,
        max_dist_horizontal: navaid.max_dist_horizontal,
        max_dist_vertical:   navaid.max_dist_vertical,
    },));
}

struct WaypointMap(HashMap<String, Entity>);

impl WaypointMap {
    fn resolve(&self, name: &str) -> Result<Entity, Error> {
        self.0.get(name).copied().ok_or_else(|| Error::UnresolvedWaypoint(name.to_string()))
    }
}

struct RoutePresetMap(HashMap<String, Entity>);

impl RoutePresetMap {
    fn resolve(&self, name: &str) -> Result<Entity, Error> {
        self.0.get(name).copied().ok_or_else(|| Error::UnresolvedRoutePreset(name.to_string()))
    }
}

fn spawn_route_presets(
    world: &mut World,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    presets: &[store::RoutePreset],
) -> Result<RoutePresetMap, Error> {
    let route_preset_entities: Vec<_> = presets
        .iter()
        .map(|preset| {
            world.spawn((store::LoadedEntity, Name::new(format!("Preset: {}", preset.id)))).id()
        })
        .collect();
    let route_preset_map = RoutePresetMap(
        presets
            .iter()
            .zip(&route_preset_entities)
            .filter_map(|(preset, &entity)| Some((preset.ref_id.clone()?, entity)))
            .collect(),
    );

    for (preset, entity) in presets.iter().zip(route_preset_entities) {
        let mut entity_ref = world.entity_mut(entity);
        entity_ref.insert(route::Preset {
            id:    preset.id.clone(),
            title: preset.title.clone(),
            nodes: convert_route(aerodromes, waypoints, &route_preset_map, &preset.nodes)
                .collect::<Result<_, Error>>()?,
        });
        match &preset.trigger {
            store::RoutePresetTrigger::Waypoint(waypoint) => {
                let waypoint = resolve_waypoint_ref(aerodromes, waypoints, waypoint)?;
                entity_ref.insert(route::PresetFromWaypoint(waypoint));
            }
        }
    }
    Ok(route_preset_map)
}

fn spawn_objects(
    world: &mut World,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    route_presets: &RoutePresetMap,
    objects: &[store::Object],
) -> Result<(), Error> {
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
) -> Result<(), Error> {
    let plane_entity = world
        .spawn((store::LoadedEntity, Name::new(format!("Plane: {}", plane.aircraft.name))))
        .id();

    let destination = match plane.aircraft.dest {
        store::Destination::Landing { ref aerodrome_code } => {
            let aerodrome = aerodromes.resolve(aerodrome_code)?;
            object::Destination::Landing { aerodrome: aerodrome.aerodrome_entity }
        }
        store::Destination::VacateAnyRunway => object::Destination::VacateAnyRunway,
        store::Destination::ReachWaypoint { min_altitude, ref waypoint_proximity } => {
            let waypoint_proximity = waypoint_proximity
                .as_ref()
                .map(|&(ref waypoint, dist)| {
                    let waypoint = resolve_waypoint_ref(aerodromes, waypoints, waypoint)?;
                    Ok((waypoint, dist))
                })
                .transpose()?;
            object::Destination::ReachWaypoint { min_altitude, waypoint_proximity }
        }
    };

    object::SpawnCommand {
        position: plane.aircraft.position.with_altitude(plane.aircraft.altitude),
        ground_speed: (plane.aircraft.ground_speed * plane.aircraft.ground_dir)
            .with_vertical(plane.aircraft.vert_rate),
        display: object::Display { name: plane.aircraft.name.clone() },
        destination,
    }
    .apply(world.entity_mut(plane_entity));

    world.entity_mut(plane_entity).insert(plane.taxi_limits.clone());

    plane::SpawnCommand {
        control: Some(plane::Control {
            heading:     plane.control.heading,
            yaw_speed:   plane.control.yaw_speed,
            horiz_accel: plane.control.horiz_accel,
        }),
        limits:  plane.nav_limits.clone(),
    }
    .apply(world.entity_mut(plane_entity));

    if let store::NavTarget::Airborne(..) = plane.nav_target {
        object::SetAirborneCommand.apply(world.entity_mut(plane_entity));

        let mut plane_ref = world.entity_mut(plane_entity);
        let airspeed =
            plane_ref.get::<object::Airborne>().expect("inserted by SetAirborneCommand").airspeed;

        let dt_target = nav::VelocityTarget {
            yaw:         nav::YawTarget::Heading(airspeed.horizontal().heading()),
            horiz_speed: airspeed.horizontal().magnitude_exact(),
            vert_rate:   Speed::ZERO,
            expedite:    false,
        };

        plane_ref.insert(dt_target);
    }

    let mut plane_ref = world.entity_mut(plane_entity);
    if let store::NavTarget::Airborne(target) = &plane.nav_target {
        insert_airborne_nav_targets(&mut plane_ref, aerodromes, waypoints, target)?;
    }

    plane_ref.insert((
        route::Id(plane.route.id.clone()),
        convert_route(aerodromes, waypoints, route_presets, &plane.route.nodes)
            .collect::<Result<Route, Error>>()?,
    ));
    route::RunCurrentNode.apply(world.entity_mut(plane_entity));

    insert_wake(world.entity_mut(plane_entity), plane);

    Ok(())
}

fn insert_airborne_nav_targets(
    plane_entity: &mut EntityWorldMut,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    target: &store::AirborneNavTarget,
) -> Result<(), Error> {
    if let Some(target_altitude) = &target.target_altitude {
        plane_entity.insert(nav::TargetAltitude {
            altitude: target_altitude.altitude,
            expedite: target_altitude.expedite,
        });
    }

    if let Some(target_glide) = &target.target_glide {
        let target_waypoint =
            resolve_waypoint_ref(aerodromes, waypoints, &target_glide.target_waypoint)?;
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
        let waypoint_entity =
            resolve_waypoint_ref(aerodromes, waypoints, &target_waypoint.waypoint)?;
        plane_entity.insert(nav::TargetWaypoint { waypoint_entity });
    }

    if let Some(target_alignment) = &target.target_alignment {
        let start_waypoint =
            resolve_waypoint_ref(aerodromes, waypoints, &target_alignment.start_waypoint)?;
        let end_waypoint =
            resolve_waypoint_ref(aerodromes, waypoints, &target_alignment.end_waypoint)?;
        plane_entity.insert(nav::TargetAlignment {
            start_waypoint,
            end_waypoint,
            lookahead: target_alignment.lookahead,
            activation_range: target_alignment.activation_range,
        });
    }

    Ok(())
}

fn convert_route<'a>(
    aerodromes: &'a AerodromeMap,
    waypoints: &'a WaypointMap,
    route_presets: &'a RoutePresetMap,
    route_nodes: &'a [store::RouteNode],
) -> impl Iterator<Item = Result<route::Node, Error>> + use<'a> {
    route_nodes
        .iter()
        .map(|node| {
            Ok(match *node {
                store::RouteNode::DirectWaypoint {
                    ref waypoint,
                    distance,
                    proximity,
                    altitude,
                } => route::node_vec(route::DirectWaypointNode {
                    waypoint: resolve_waypoint_ref(aerodromes, waypoints, waypoint)?,
                    distance,
                    proximity,
                    altitude,
                }),
                store::RouteNode::SetAirSpeed { goal, error } => {
                    route::node_vec(route::SetAirspeedNode { speed: goal, error })
                }
                store::RouteNode::StartPitchToAltitude { goal, error, expedite } => {
                    route::node_vec(route::StartSetAltitudeNode { altitude: goal, error, expedite })
                }
                store::RouteNode::RunwayLanding { ref runway, ref goaround_preset } => {
                    let runway = resolve_runway_ref(aerodromes, runway)?.runway;
                    let goaround_preset = if let Some(goaround_preset) = goaround_preset {
                        Some(route_presets.resolve(goaround_preset)?)
                    } else {
                        None
                    };
                    Vec::<route::Node>::from([
                        route::AlignRunwayNode { runway, expedite: true, goaround_preset }.into(),
                        route::ShortFinalNode { runway, goaround_preset }.into(),
                        route::VisualLandingNode { runway, goaround_preset }.into(),
                    ])
                }
            })
        })
        .flat_map(|result| match result {
            Ok(nodes) => Either::Left(nodes.into_iter().map(Ok)),
            Err(err) => Either::Right(iter::once(Err(err))),
        })
}

fn insert_wake(mut plane_entity: EntityWorldMut, plane: &store::Plane) {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // nearest positive integer
    let base_intensity = wake::Intensity(
        (WAKE_FACTOR * plane.aircraft.weight / plane.aircraft.wingspan.into_nm()) as u32,
    );
    plane_entity.insert((wake::Producer { base_intensity }, wake::Detector::default()));
}

fn resolve_runway_ref<'a>(
    aerodromes: &'a AerodromeMap,
    store::RunwayRef { aerodrome_code, runway_name }: &store::RunwayRef,
) -> Result<&'a SpawnedRunway, Error> {
    let aerodrome = aerodromes.resolve(aerodrome_code)?;
    let Some(runway) = aerodrome.runways.get(runway_name) else {
        return Err(Error::UnresolvedRunway {
            aerodrome: aerodrome_code.clone(),
            runway:    runway_name.clone(),
        });
    };
    Ok(runway)
}

fn resolve_waypoint_ref(
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    waypoint_ref: &store::WaypointRef,
) -> Result<Entity, Error> {
    match waypoint_ref {
        store::WaypointRef::Named(name) => waypoints.resolve(name),
        store::WaypointRef::RunwayThreshold(runway_ref) => {
            Ok(resolve_runway_ref(aerodromes, runway_ref)?.runway)
        }
        store::WaypointRef::LocalizerStart(runway_ref) => {
            Ok(resolve_runway_ref(aerodromes, runway_ref)?.localizer_waypoint)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Deserialization error: {0}")]
    Serde(ciborium::de::Error<io::Error>),
    #[error("Too many aerodromes")]
    TooManyAerodromes,
    #[error("No aerodrome called {0:?}")]
    UnresolvedAerodrome(String),
    #[error("No runway called {runway:?} in aerodrome {aerodrome:?}")]
    UnresolvedRunway { aerodrome: String, runway: String },
    #[error("No waypoint called {0:?}")]
    UnresolvedWaypoint(String),
    #[error("No route preset called {0:?}")]
    UnresolvedRoutePreset(String),
    #[error("Non-finite value encountered at {0}")]
    NonFiniteFloat(&'static str),
    #[error(
        "The backward direction of apron {0} does not intersect with any taxiways within 100nm"
    )]
    UnreachableApron(String),
    #[error("Resolve ground lines: {0}")]
    GroundSweep(sweep::Error),
}

type VecResult<T> = Result<Vec<T>, Error>;
type HashMapResult<K, V> = Result<HashMap<K, V>, Error>;

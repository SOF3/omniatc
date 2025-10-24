use std::collections::HashMap;
use std::f32::consts;

use bevy::ecs::entity::Entity;
use bevy::ecs::name::Name;
use bevy::ecs::relationship::{RelatedSpawner, Relationship};
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::World;
use bevy::math::Vec2;
use itertools::Itertools;
use math::sweep::{self, LineSweeper};
use math::{Angle, Heading, Length, Position, Speed};
use ordered_float::OrderedFloat;

use super::Aerodrome;
use crate::level::navaid::{self, Navaid};
use crate::level::runway::{self, Runway};
use crate::level::waypoint::{self, Waypoint};
use crate::level::{aerodrome, ground};
use crate::load::{self, StoredEntity};

pub const APRON_FORWARD_HEADING_DIRECTION: ground::SegmentDirection =
    ground::SegmentDirection::BetaToAlpha;

/// Spawns the aerodromes declared in a store.
///
/// # Errors
/// If the stored aerodromes contain invalid references.
pub fn spawn(
    world: &mut World,
    aerodromes: &[store::Aerodrome],
) -> Result<AerodromeMap, load::Error> {
    aerodromes
        .iter()
        .enumerate()
        .map(|(id, aerodrome)| {
            let aerodrome_id = u32::try_from(id).map_err(|_| load::Error::TooManyAerodromes)?;
            let aerodrome_entity = world
                .spawn((
                    StoredEntity,
                    Name::new(format!("Aerodrome: {}", aerodrome.code)),
                    Aerodrome {
                        id:        aerodrome_id,
                        code:      aerodrome.code.clone(),
                        name:      aerodrome.full_name.clone(),
                        elevation: aerodrome.elevation,
                    },
                ))
                .id();
            world.write_message(aerodrome::SpawnMessage(aerodrome_entity));

            let mut runway_entities = HashMap::new();
            for runway_pair in &aerodrome.runways {
                let forward = spawn_runway(
                    world,
                    aerodrome,
                    runway_pair.width,
                    &runway_pair.forward,
                    runway_pair.forward_start,
                    runway_pair.backward_start,
                    aerodrome_entity,
                );
                let backward = spawn_runway(
                    world,
                    aerodrome,
                    runway_pair.width,
                    &runway_pair.backward,
                    runway_pair.backward_start,
                    runway_pair.forward_start,
                    aerodrome_entity,
                );
                runway_entities.insert(
                    runway_pair.forward.name.clone(),
                    PairedSpawnedRunway {
                        runway:    forward,
                        paired:    backward.runway,
                        direction: ground::SegmentDirection::AlphaToBeta,
                    },
                );
                runway_entities.insert(
                    runway_pair.backward.name.clone(),
                    PairedSpawnedRunway {
                        runway:    backward,
                        paired:    forward.runway,
                        direction: ground::SegmentDirection::BetaToAlpha,
                    },
                );
            }

            let spawned_segments = spawn_ground_segments(
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
                    spawned_segments,
                },
            ))
        })
        .collect::<load::HashMapResult<_, _>>()
        .map(AerodromeMap)
}

/// Stores the mapping from loaded aerodromes to their spawned entities.
pub struct AerodromeMap(HashMap<String, SpawnedAerodrome>);

impl AerodromeMap {
    /// Resolves the stored aerodrome reference and loads the relevant data.
    ///
    /// # Errors
    /// If the referenced aerodrome does not exist.
    pub fn resolve(&self, code: &store::AerodromeRef) -> Result<&SpawnedAerodrome, load::Error> {
        self.0.get(&code.0).ok_or_else(|| load::Error::UnresolvedAerodrome(code.0.clone()))
    }

    /// Resolves the stored segment reference
    /// into a runtime segment label reference.
    ///
    /// # Errors
    /// If the referenced aerodrome or runway does not exist.
    pub fn resolve_segment(
        &self,
        segment: &store::SegmentRef,
    ) -> Result<ground::SegmentLabel, load::Error> {
        Ok(match &segment.label {
            store::SegmentLabel::Taxiway(name) => {
                ground::SegmentLabel::Taxiway { name: name.clone() }
            }
            store::SegmentLabel::Apron(name) => ground::SegmentLabel::Apron { name: name.clone() },
            store::SegmentLabel::Runway(runway) => {
                let runway = self.resolve_runway_ref_destructured(&segment.aerodrome, runway)?;
                ground::SegmentLabel::RunwayPair([runway.runway.runway, runway.paired])
            }
        })
    }

    /// Resolves the actual segment from the label reference
    /// by selecting the segment most parallel to the given heading
    /// among those matching the label and containing the position within its width from centerline.
    ///
    /// # Errors
    /// If the referenced aerodrome or runway does not exist.
    pub fn resolve_closest_segment_by_label(
        &self,
        segment_ref: &store::SegmentRef,
        position: Position<Vec2>,
        heading: Heading,
        object_name: &str,
    ) -> Result<(Entity, ground::SegmentDirection), load::Error> {
        let aerodrome = self.resolve(&segment_ref.aerodrome)?;

        let label = match &segment_ref.label {
            store::SegmentLabel::Taxiway(name) => {
                ground::SegmentLabel::Taxiway { name: name.clone() }
            }
            store::SegmentLabel::Apron(name) => ground::SegmentLabel::Apron { name: name.clone() },
            store::SegmentLabel::Runway(runway) => {
                let runway =
                    self.resolve_runway_ref_destructured(&segment_ref.aerodrome, runway)?;
                ground::SegmentLabel::RunwayPair([runway.runway.runway, runway.paired])
            }
        };

        let possible_segments = aerodrome.spawned_segments.get(&label).ok_or_else(|| {
            load::Error::UnresolvedSegment {
                variant:   (&segment_ref.label).into(),
                value:     segment_ref.label.inner_name().to_owned(),
                aerodrome: segment_ref.aerodrome.0.clone(),
            }
        })?;

        let heading_dir = heading.into_dir2();

        let closest = possible_segments
            .iter()
            .filter(|segment| {
                math::point_segment_closest(position, segment.alpha_position, segment.beta_position)
                    .distance_cmp(position)
                    < segment.width / 2.0
            })
            .map(|segment| {
                let segment_heading =
                    (segment.beta_position - segment.alpha_position).0.normalize_or_zero();
                (segment.entity, segment_heading.dot(*heading_dir))
            })
            .max_by_key(|&(_entity, dot)| OrderedFloat(dot.abs()));
        match closest {
            None => Err(load::Error::NotOnSegment {
                object:    object_name.to_owned(),
                variant:   (&segment_ref.label).into(),
                value:     segment_ref.label.inner_name().to_owned(),
                aerodrome: segment_ref.aerodrome.0.clone(),
            }),
            Some((segment_entity, dot)) => {
                if dot >= 0.0 {
                    Ok((segment_entity, ground::SegmentDirection::AlphaToBeta))
                } else {
                    Ok((segment_entity, ground::SegmentDirection::BetaToAlpha))
                }
            }
        }
    }

    /// Resolves the stored runway reference into a runtime runway reference.
    ///
    /// # Errors
    /// If the referenced aerodrome or runway does not exist.
    pub fn resolve_runway_ref<'a>(
        &'a self,
        runway: &store::RunwayRef,
    ) -> Result<&'a PairedSpawnedRunway, load::Error> {
        self.resolve_runway_ref_destructured(&runway.aerodrome, &runway.runway_name)
    }

    fn resolve_runway_ref_destructured<'a>(
        &'a self,
        aerodrome_ref: &store::AerodromeRef,
        runway_name: &str,
    ) -> Result<&'a PairedSpawnedRunway, load::Error> {
        let aerodrome = self.resolve(aerodrome_ref)?;
        let Some(runway) = aerodrome.runways.get(runway_name) else {
            return Err(load::Error::UnresolvedRunway {
                aerodrome: aerodrome_ref.0.clone(),
                runway:    runway_name.to_string(),
            });
        };
        Ok(runway)
    }
}

pub struct SpawnedAerodrome {
    pub aerodrome_entity: Entity,
    pub index:            u32,
    pub runways:          HashMap<String, PairedSpawnedRunway>,
    pub spawned_segments: SpawnedSegments,
}

#[derive(Clone, Copy)]
pub struct SpawnedRunway {
    pub runway:             Entity,
    pub start_pos:          Position<Vec2>,
    pub localizer_waypoint: Entity,
}

/// A pair of runway entities on opposite directions for the same physical runway.
pub struct PairedSpawnedRunway {
    /// The runway entity resolved directly.
    pub runway:    SpawnedRunway,
    /// The other runway in the pair.
    pub paired:    Entity,
    /// The segment direction of the matched runway.
    pub direction: ground::SegmentDirection,
}

impl PairedSpawnedRunway {
    #[must_use]
    pub fn to_segment_label(&self) -> ground::SegmentLabel {
        ground::SegmentLabel::RunwayPair([self.runway.runway, self.paired])
    }
}

fn spawn_runway(
    world: &mut World,
    aerodrome: &store::Aerodrome,
    runway_width: Length<f32>,
    runway: &store::Runway,
    start_pos: Position<Vec2>,
    end_pos: Position<Vec2>,
    aerodrome_entity: Entity,
) -> SpawnedRunway {
    let runway_entity = world
        .spawn((StoredEntity, Name::new(format!("Runway: {}/{}", aerodrome.code, runway.name))))
        .id();

    let heading = Heading::from_vec2((end_pos - start_pos).0);
    let touchdown_position =
        (start_pos + runway.touchdown_displacement * heading).with_altitude(aerodrome.elevation);

    runway::SpawnCommand {
        waypoint:  Waypoint {
            name:         runway.name.clone(),
            display_type: waypoint::DisplayType::Runway,
            position:     touchdown_position,
        },
        runway:    Runway {
            landing_length: (end_pos - start_pos).normalize_to_magnitude(
                start_pos.distance_exact(end_pos) - runway.touchdown_displacement,
            ),
            glide_descent:  runway.glide_angle,
            display_start:  start_pos.with_altitude(aerodrome.elevation),
            display_end:    end_pos.with_altitude(aerodrome.elevation),
            width:          runway_width,
        },
        aerodrome: aerodrome_entity,
    }
    .apply(world.entity_mut(runway_entity));

    world.entity_mut(runway_entity).with_related_entities::<navaid::OwnerWaypoint>(|b| {
        spawn_runway_navaids(b, heading, runway);
    });

    let localizer_waypoint = world
        .spawn((
            StoredEntity,
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

    SpawnedRunway { runway: runway_entity, start_pos, localizer_waypoint }
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
            min_dist_horizontal: Length::ZERO,
            min_dist_vertical:   Length::ZERO,
            // TODO overwrite these two fields with visibility
            max_dist_horizontal: runway.max_visual_distance,
            max_dist_vertical:   Length::from_km(10.),
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

const GROUND_EPSILON: Length<f32> = Length::from_meters(1.);

fn collect_non_apron_ground_lines(
    ground_network: &store::GroundNetwork,
    runway_pairs: &[store::RunwayPair],
    runways: &HashMap<String, PairedSpawnedRunway>,
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
                label:     ground::SegmentLabel::RunwayPair([
                    forward_runway.runway.runway,
                    backward_runway.runway.runway,
                ]),
                width:     pair.width,
                max_speed: ground_network.taxi_speed,
                alpha:     pair.forward_start
                    + (pair.forward_start - pair.backward_start)
                        .normalize_to_magnitude(pair.backward.stopway),
                beta:      pair.backward_start
                    + (pair.backward_start - pair.forward_start)
                        .normalize_to_magnitude(pair.forward.stopway),
            }
        })
        .chain(ground_network.taxiways.iter().flat_map(|taxiway| {
            taxiway.endpoints.iter().tuple_windows().map(|(&alpha, &beta)| GroundLine {
                label: ground::SegmentLabel::Taxiway { name: taxiway.name.clone() },
                width: taxiway.width,
                max_speed: ground_network.taxi_speed,
                alpha,
                beta,
            })
        }))
        .collect()
}

fn generate_apron_lines(
    ground_network: &store::GroundNetwork,
    lines: &mut Vec<GroundLine>,
) -> Result<(), load::Error> {
    let mut aprons: Vec<_> = ground_network
        .aprons
        .iter()
        .map(|apron| {
            let heading = apron
                .forward_heading
                .opposite()
                .as_ordered()
                .map_err(|_| load::Error::NonFiniteFloat("apron forward_heading"))?;
            Ok((heading, apron))
        })
        .collect::<Result<_, load::Error>>()?;
    aprons.sort_by_key(|apron| apron.0);

    let non_apron_lines_len = lines.len();
    lines.extend(aprons.iter().map(|(_, apron)| GroundLine {
        label:     ground::SegmentLabel::Apron { name: apron.name.clone() },
        width:     apron.width,
        max_speed: ground_network.apron_speed,
        alpha:     apron.position,
        beta:      apron.position, // we will update this later
    }));
    let (non_apron_lines, apron_lines) = lines.split_at_mut(non_apron_lines_len);

    for (apron_index, &(_apron_back, apron)) in aprons.iter().enumerate() {
        let sweeper = LineSweeper::new(
            |index| match index.0.checked_sub(1) {
                None => sweep::Line {
                    alpha:          apron.position,
                    beta:           apron.position - Length::from_nm(100.) * apron.forward_heading,
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
        .map_err(load::Error::GroundSweep)?;

        let intersect = sweeper
            .intersections_after(apron.position)
            .next()
            .ok_or_else(|| load::Error::UnreachableApron(apron.name.clone()))?;
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

fn find_ground_intersects(lines: &[GroundLine]) -> Result<Vec<IntersectGroup>, load::Error> {
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
        Length::from_meters(0.1),
        Heading::from_radians(Angle::new(consts::E)).into_dir2(), // an arbitrary direction to avoid duplicates
    )
    .map_err(load::Error::GroundSweep)?
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
    alpha:         GroundSegmentEndpoint,
    beta:          GroundSegmentEndpoint,
    line:          sweep::LineIndex,
    display_label: bool,
}

#[derive(Clone, Copy)]
struct GroundSegmentEndpoint {
    position: Position<Vec2>,
    group:    Option<usize>,
}

fn ground_lines_to_segments(
    lines: &[GroundLine],
    all_intersect_groups: &[IntersectGroup],
) -> Vec<GroundSegment> {
    let mut line_to_intersects_map = HashMap::<_, Vec<_>>::new();
    for group in all_intersect_groups {
        for line in &group.lines {
            let alpha_dist = group.position.distance_cmp(lines[line.0].alpha);
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

        let endpoint_pairs = alpha_endpoint
            .into_iter()
            .chain(intersects.iter().map(|&(_, group)| GroundSegmentEndpoint {
                position: group.position,
                group:    Some(group.index),
            }))
            .chain(beta_endpoint)
            .tuple_windows();

        let display_index = line_strip_midpoint_index(
            endpoint_pairs
                .clone()
                .map(|(alpha, beta)| alpha.position.distance_exact(beta.position)),
        )
        .unwrap_or(intersects.len() + 1); // chain(alpha, intersects, beta).tuple_windows()

        segments.extend(endpoint_pairs.enumerate().map(|(index, (alpha, beta))| GroundSegment {
            alpha,
            beta,
            line: line_index,
            display_label: index == display_index,
        }));
    }

    segments
}

fn line_strip_midpoint_index(
    endpoint_pairs: impl Iterator<Item = Length<f32>> + Clone,
) -> Option<usize> {
    // I could have summed from both ends until the two iterators meet in the middle,
    // but that is more complex and usually not worth optimizing
    // given the expected small number of segments per line.

    let total_length: Length<f32> = endpoint_pairs.clone().sum();
    let mut prefix_sum = endpoint_pairs.scan(Length::ZERO, |sum, length| {
        *sum += length;
        Some(*sum)
    });
    prefix_sum.position(|sum| sum * 2.0 >= total_length)
}

pub type SpawnedSegments = HashMap<ground::SegmentLabel, Vec<SpawnedSegment>>;

pub struct SpawnedSegment {
    pub entity:         Entity,
    pub alpha_position: Position<Vec2>,
    pub beta_position:  Position<Vec2>,
    pub width:          Length<f32>,
}

fn spawn_ground_segments(
    world: &mut World,
    ground_network: &store::GroundNetwork,
    runway_pairs: &[store::RunwayPair],
    runways: &HashMap<String, PairedSpawnedRunway>,
    aerodrome_entity: Entity,
    elevation: Position<f32>,
) -> Result<SpawnedSegments, load::Error> {
    let mut lines = collect_non_apron_ground_lines(ground_network, runway_pairs, runways);
    generate_apron_lines(ground_network, &mut lines)?;
    let intersect_groups = find_ground_intersects(&lines)?;
    let segments = ground_lines_to_segments(&lines, &intersect_groups);

    let endpoints: Vec<_> = intersect_groups
        .iter()
        .map(|group| {
            let entity = world.spawn_empty().id();
            ground::SpawnEndpoint { position: group.position, aerodrome: aerodrome_entity }
                .apply(world.entity_mut(entity));
            entity
        })
        .collect();

    let mut spawned_segments = SpawnedSegments::new();

    for segment in segments {
        let [alpha_endpoint, beta_endpoint] = [&segment.alpha, &segment.beta].map(|endpoint| {
            if let Some(index) = endpoint.group {
                endpoints[index]
            } else {
                // spawn a non-intersecting endpoint only used in this segment
                let entity = world.spawn_empty().id();
                ground::SpawnEndpoint { position: endpoint.position, aerodrome: aerodrome_entity }
                    .apply(world.entity_mut(entity));
                entity
            }
        });

        let segment_entity = world.spawn_empty().id();
        let &GroundLine { ref label, width, max_speed, .. } = &lines[segment.line.0];
        ground::SpawnSegment {
            segment:       ground::Segment {
                alpha: alpha_endpoint,
                beta: beta_endpoint,
                width,
                max_speed,
                elevation,
            },
            label:         label.clone(),
            aerodrome:     aerodrome_entity,
            display_label: segment.display_label,
        }
        .apply(world.entity_mut(segment_entity));

        spawned_segments.entry(label.clone()).or_default().push(SpawnedSegment {
            entity: segment_entity,
            alpha_position: segment.alpha.position,
            beta_position: segment.beta.position,
            width,
        });
    }

    Ok(spawned_segments)
}

struct GroundLine {
    label:     ground::SegmentLabel,
    width:     Length<f32>,
    max_speed: Speed<f32>,
    alpha:     Position<Vec2>,
    beta:      Position<Vec2>,
}

impl GroundLine {
    /// Returns a reference to the endpoint position
    /// representing the intersection point of an apron with the nearest taxiway.
    fn apron_intersect_mut(&mut self) -> &mut Position<Vec2> {
        match APRON_FORWARD_HEADING_DIRECTION {
            ground::SegmentDirection::AlphaToBeta => &mut self.alpha,
            ground::SegmentDirection::BetaToAlpha => &mut self.beta,
        }
    }
}

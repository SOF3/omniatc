use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, ResMut, SystemParam};
use bevy::math::Dir2;
use math::{Length, Position, Speed, rotate_clockwise, segment_segment_distance};

use crate::level::object::Object;
use crate::level::runway::Runway;
use crate::level::waypoint::Waypoint;
use crate::level::{ground, object, runway, score, taxi};
use crate::{QueryTryLog, try_log};

/// Speed below which an object is considered to be stationary.
const MOVING_THRESHOLD: Speed<f32> = Speed::from_meter_per_sec(0.1);

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Update, completion_system.in_set(score::Writer));
    }
}

/// Objective for the flight.
/// Applied on objects that should be despawned upon reaching the goal.
#[derive(Component)]
pub enum Destination {
    /// Object can be handed over upon vacating a runway in the specific aerodrome.
    Landing { aerodrome: Entity },
    /// Object can be handed over upon parking in an apron in the specific aerodrome.
    Parking { aerodrome: Entity },
    /// Object can be handed over upon vacating any runway.
    VacateAnyRunway,
    // TODO: apron/taxiway arrival.
    /// Reach a given waypoint and a given altitude.
    ///
    /// Either condition is set to `None` upon completion.
    /// The control of the object is completed when both are `None`.
    Departure {
        min_altitude:       Option<Position<f32>>,
        waypoint_proximity: Option<(Entity, Length<f32>)>,
    },
}

/// Objects with this component award a score upon completion of their destination.
#[derive(Component)]
pub struct CompletionScore {
    /// Score delta for each completed arrival or departure.
    pub delta: score::Unit,
}

fn completion_system(
    object_query: Query<(CompletionObjectQuery, &mut Destination, Option<&CompletionScore>)>,
    params: CompletionParams,
    mut commands: Commands,
    mut score: ResMut<score::Scores>,
) {
    let mut arrivals = 0;
    let mut departures = 0;
    let mut delta = score::Unit::default();

    for (object, mut dest, reward) in object_query {
        let result = match *dest {
            Destination::Landing { aerodrome } => {
                detect_runway_vacation(&object, Some(aerodrome), &params)
            }
            Destination::Parking { aerodrome } => {
                detect_apron_stop(&object, Some(aerodrome), &params)
            }
            Destination::VacateAnyRunway => detect_runway_vacation(&object, None, &params),
            Destination::Departure { ref mut min_altitude, ref mut waypoint_proximity } => {
                detect_departure(&object, min_altitude, waypoint_proximity, &params)
            }
        };
        if let Some(DetectResult::Completed) = result {
            commands.entity(object.entity).queue(object::DespawnCommand);

            match *dest {
                Destination::Landing { .. }
                | Destination::Parking { .. }
                | Destination::VacateAnyRunway => {
                    arrivals += 1;
                }
                Destination::Departure { .. } => {
                    departures += 1;
                }
            }

            if let Some(reward) = reward {
                delta += reward.delta;
            }
        }
    }

    score.num_arrivals += arrivals;
    score.num_departures += departures;
    score.total += delta;
}

#[derive(QueryData)]
struct CompletionObjectQuery {
    entity:      Entity,
    object:      &'static Object,
    ground:      Option<(&'static object::OnGround, &'static object::TaxiStatus)>,
    taxi_limits: &'static taxi::Limits,
}

#[derive(SystemParam)]
#[expect(clippy::struct_field_names)] // all queries by coincidence
struct CompletionParams<'w, 's> {
    waypoint_query:  Query<'w, 's, &'static Waypoint>,
    segment_query: Query<
        'w,
        's,
        (&'static ground::Segment, &'static ground::SegmentLabel, &'static ground::SegmentOf),
    >,
    endpoint_query:  Query<'w, 's, &'static ground::Endpoint>,
    aerodrome_query: Query<'w, 's, &'static runway::AerodromeRunways>,
    runway_query:    Query<'w, 's, (&'static Waypoint, &'static Runway)>,
}

enum DetectResult {
    Completed,
    Incomplete,
}

fn detect_runway_vacation(
    object: &CompletionObjectQueryItem,
    want_aerodrome: Option<Entity>,
    params: &CompletionParams<'_, '_>,
) -> Option<DetectResult> {
    let Some((ground, taxi_status)) = object.ground else {
        return Some(DetectResult::Incomplete);
    };

    let (_, label, object_aerodrome) = params.segment_query.log_get(ground.segment)?;
    if want_aerodrome.is_some_and(|a| a != object_aerodrome.0) {
        return Some(DetectResult::Incomplete);
    }
    if let ground::SegmentLabel::RunwayPair(_) = label {
        return Some(DetectResult::Incomplete);
    }

    // The object is now taxiing on a non-runway segment.
    // However, we need to check if it has vacated the runway fully.
    // Considering that the number of runways within an aerodrome is typically small,
    // we just iterate over all runway segments in the aerodrome
    // and test if the object heading and length fully vacates all of them.
    let object_half_dir = object.taxi_limits.half_length * taxi_status.heading;
    let object_head = object.object.position.horizontal() + object_half_dir;

    let runways = params.aerodrome_query.log_get(object_aerodrome.0)?;
    for &runway_id in runways.as_ref() {
        let (waypoint, runway) = params.runway_query.log_get(runway_id)?;
        let closest = segment_segment_distance(
            waypoint.position.horizontal(),
            runway.landing_length,
            object_head,
            object_half_dir * -2.0,
        );
        let ortho_dir = try_log!(
            Dir2::new(rotate_clockwise(runway.landing_length.0)),
            expect "runway landing length should be nonzero" or return None
        );
        if closest.project_onto_dir(ortho_dir).abs() < runway.width * 0.5 {
            return Some(DetectResult::Incomplete);
        }
    }
    Some(DetectResult::Completed)
}

fn detect_apron_stop(
    object: &CompletionObjectQueryItem,
    want_aerodrome: Option<Entity>,
    params: &CompletionParams<'_, '_>,
) -> Option<DetectResult> {
    let Some((ground, _)) = object.ground else {
        return Some(DetectResult::Incomplete);
    };

    if object.object.ground_speed.magnitude_cmp() > MOVING_THRESHOLD {
        return Some(DetectResult::Incomplete);
    }

    let (_, label, object_aerodrome) = params.segment_query.log_get(ground.segment)?;
    if want_aerodrome.is_some_and(|a| a != object_aerodrome.0) {
        return Some(DetectResult::Incomplete);
    }
    if let ground::SegmentLabel::Apron { .. } = label {
        Some(DetectResult::Completed)
    } else {
        Some(DetectResult::Incomplete)
    }
}

fn detect_departure(
    object: &CompletionObjectQueryItem,
    min_altitude: &mut Option<Position<f32>>,
    waypoint_proximity: &mut Option<(Entity, Length<f32>)>,
    params: &CompletionParams<'_, '_>,
) -> Option<DetectResult> {
    if let Some((waypoint_entity, proximity)) = *waypoint_proximity {
        let waypoint = params.waypoint_query.log_get(waypoint_entity)?;
        if object.object.position.horizontal_distance_cmp(waypoint.position) <= proximity {
            *waypoint_proximity = None;
        }
    }

    if let Some(min_alt) = *min_altitude
        && object.object.position.altitude() >= min_alt
    {
        *min_altitude = None;
    }

    Some(if min_altitude.is_none() && waypoint_proximity.is_none() {
        DetectResult::Completed
    } else {
        DetectResult::Incomplete
    })
}

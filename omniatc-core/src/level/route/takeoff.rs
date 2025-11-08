use std::time::Duration;

use bevy::ecs::entity::Entity;
use bevy::ecs::system::{EntityCommand, SystemState};
use bevy::ecs::world::World;
use bevy::time::{self, Time};
use math::Position;
use store::{ClimbProfile, NavLimits, TaxiLimits};

use crate::level::object::{GroundSpeedCalculator, Object};
use crate::level::route::{NodeKind, RunNodeResult, TaxiNode, TaxiStopMode, trigger};
use crate::level::runway::Runway;
use crate::level::waypoint::Waypoint;
use crate::level::{ground, message, nav, object, taxi};
use crate::{EntityTryLog, WorldTryLog};

/// Accelerate to takeoff speed and set the object to airborne.
///
/// # Completion condition
/// Completes when the object is airborne.
///
/// Takeoff may abort due to the following reasons:
///
/// - Insufficient runway length.
/// - Runway not clear.
/// - Unsafe crosswind.
///
/// When takeoff aborts, target taxi speed is set to 0,
/// and subsequent nodes in the object route are cleared.
/// The current node completes when taxi speed reduces to a negligible value.
///
/// # Prerequisites
/// The object must be on ground and on a runway segment.
#[derive(Clone, Copy)]
pub struct TakeoffNode {
    /// The initial altitude clearance after takeoff.
    pub target_altitude: Position<f32>,
}

impl NodeKind for TakeoffNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        run_takeoff_node(world, entity, *self).unwrap_or(RunNodeResult::PendingTrigger)
    }
}

fn run_takeoff_node(world: &mut World, entity: Entity, node: TakeoffNode) -> Option<RunNodeResult> {
    let mut gs_calc = SystemState::<GroundSpeedCalculator>::new(world);

    let object = world.entity(entity);

    let NavLimits {
        takeoff_speed: takeoff_airspeed,
        std_climb: ClimbProfile { vert_rate: climb_rate, .. },
        ..
    } = object.log_get::<nav::Limits>()?.0;
    let TaxiLimits { accel, .. } = object.log_get::<taxi::Limits>()?.0;

    let &Object { position: object_pos, ground_speed: current_ground_speed } = object.log_get()?;

    let &object::OnGround { segment: segment_id, direction, .. } = object.log_get()?;
    let segment_entity = world.entity(segment_id);
    let Some(runways) = segment_entity.get::<ground::SegmentOfRunway>() else {
        bevy::log::error!(
            "TakeoffNode expects object {entity:?} to be on a runway segment, is on {:?} instead",
            segment_entity.id()
        );
        return None;
    };
    let runway_entity = runways.by_direction(direction);
    let runway_touchdown_position = world.log_get::<Waypoint>(runway_entity)?.position.horizontal();
    let runway = world.log_get::<Runway>(runway_entity)?;
    let runway_dir = runway.landing_length;
    let tora_end = runway_touchdown_position + runway.landing_length;

    let takeoff_ground_speed = gs_calc
        .get(world)
        .get_ground_speed(
            object_pos,
            runway_dir.normalize_to_magnitude(takeoff_airspeed).horizontally(),
        )
        .ground_speed;

    if current_ground_speed.magnitude_cmp() > takeoff_ground_speed.magnitude_cmp() {
        // Takeoff speed reached, set object to airborne.

        object::SetAirborneCommand.apply(world.entity_mut(entity));

        let mut object_ref = world.entity_mut(entity);
        let airspeed =
            object_ref.get::<object::Airborne>().expect("inserted by SetAirborneCommand").airspeed;

        object_ref.insert((
            nav::VelocityTarget {
                yaw:         store::YawTarget::Heading(airspeed.horizontal().heading()),
                horiz_speed: airspeed.horizontal().magnitude_exact(),
                vert_rate:   climb_rate,
                expedite:    false,
            },
            nav::TargetAltitude { altitude: node.target_altitude, expedite: false },
        ));

        return Some(RunNodeResult::NodeDone);
    }

    // v^2 = u^2 + 2as => s = (v^2 - u^2) / 2a
    let required_dist =
        (takeoff_ground_speed.magnitude_squared() - current_ground_speed.magnitude_squared()) * 0.5
            / accel;
    let available_dist = tora_end.distance_exact(object_pos.horizontal());

    if available_dist < required_dist {
        let runway_label = ground::SegmentLabel::RunwayPair(runways.0);

        world.spawn(message::Message {
            source:  entity,
            created: world.resource::<Time<time::Virtual>>().elapsed(),
            content: format!(
                "Takeoff aborted due to insufficient runway length (required {:.0} m, available \
                 {:.0} m)",
                required_dist.into_meters(),
                available_dist.into_meters()
            ),
            class:   message::Class::AnomalyInfo,
        });

        return Some(RunNodeResult::ReplaceWithNodes(
            [TaxiNode {
                label:     runway_label,
                direction: None,
                stop:      TaxiStopMode::LineUp,
            }
            .into()]
            .into(),
        ));
    }

    world.entity_mut(entity).insert((
        taxi::Target {
            action:     taxi::TargetAction::Takeoff { runway: runway_entity },
            resolution: None,
        },
        trigger::TimeDelay(Duration::from_secs(1)),
    ));

    Some(RunNodeResult::PendingTrigger)
}

use std::f32::consts::FRAC_PI_8;
use std::time::Duration;

use bevy::ecs::system::Command;
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Dir2, Vec2};
use bevy::prelude::{Entity, EntityCommand, EntityRef, World};
use bevy::time::{self, Time};
use math::{Angle, Distance, Heading, Speed};

use super::{trigger, HorizontalTarget, NodeKind, RunNodeResult};
use crate::level::object::{self, Object};
use crate::level::runway::{self, Runway};
use crate::level::waypoint::Waypoint;
use crate::level::{ground, message, nav, navaid, taxi};
use crate::{try_log, try_log_return};

/// [Activation range](nav::TargetAlignment::activation_range) for `AlignRunway` nodes.
///
/// This constant has relatively longer activation range
/// compared to the default one triggered by explicit user command,
/// because the object is expected to immediately start aligning
/// by the time the `AlignRunway` node becomes active.
const ALIGN_RUNWAY_ACTIVATION_RANGE: Distance<f32> = Distance::from_nm(0.5);

/// [Lookahead duration](nav::TargetAlignment::lookahead) for `AlignRunway` nodes.
const ALIGN_RUNWAY_LOOKAHEAD: Duration = Duration::from_secs(10);

const MAX_TRACK_DEVIATION: Angle<f32> = Angle(FRAC_PI_8);

fn align_runway(object: &mut EntityWorldMut, runway: Entity, expedite: bool) -> Result<(), ()> {
    let Some((glide_descent, localizer_waypoint)) = object.world_scope(|world| {
        Some((
            try_log!(
                world.get::<Runway>(runway),
                expect "AlignRunwayNode references non-runway entity {runway:?}"
                or return None
            )
            .glide_descent,
            try_log!(
                world.get::<runway::LocalizerWaypointRef>(runway),
                expect "Runway {runway:?} has no LocalizerWaypointRef"
                or return None
            )
            .localizer_waypoint,
        ))
    }) else {
        return Err(());
    };

    object.remove::<(nav::TargetWaypoint, nav::TargetAltitude)>().insert((
        nav::TargetAlignment {
            start_waypoint:   localizer_waypoint,
            end_waypoint:     runway,
            activation_range: ALIGN_RUNWAY_ACTIVATION_RANGE,
            lookahead:        ALIGN_RUNWAY_LOOKAHEAD,
        },
        nav::TargetGlide {
            target_waypoint: runway,
            glide_angle: -glide_descent,
            // the actual minimum pitch is regulated by maximum descent rate.
            min_pitch: -Angle::RIGHT,
            max_pitch: Angle::ZERO,
            lookahead: ALIGN_RUNWAY_LOOKAHEAD,
            expedite,
        },
    ));

    Ok(())
}

/// Aligns the object with a runway localizer during the final leg,
/// before switching to short final.
///
/// Short final here is defined as the point at which
/// the object must start reducing to threshold crossing speed.
///
/// Must be followed by [`ShortFinalNode`].
/// Completes when distance from runway is less than
/// [`nav::Limits::short_final_dist`].
#[derive(Clone, Copy)]
pub struct AlignRunwayNode {
    /// The runway waypoint entity.
    pub runway:          Entity,
    /// Whether to allow descent expedition to align with the glidepath.
    pub expedite:        bool,
    /// The preset to switch to in case of missed approach.
    pub goaround_preset: Option<Entity>,
}

impl NodeKind for AlignRunwayNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        let &Waypoint { position: runway_position, .. } = try_log!(
            world.get::<Waypoint>(self.runway),
            expect "Runway {:?} must have a corresponding waypoint" (self.runway)
            or return RunNodeResult::PendingTrigger
        );

        let mut object = world.entity_mut(entity);
        if align_runway(&mut object, self.runway, self.expedite).is_err() {
            return RunNodeResult::PendingTrigger;
        }

        let position = object.get::<Object>().expect("entity must be an Object").position;
        let limits = try_log!(
            object.get::<nav::Limits>(),
            expect "Landing aircraft must have nav limits"
            or return RunNodeResult::PendingTrigger
        );
        let dist = position.horizontal_distance_exact(runway_position);
        if dist < limits.short_final_dist {
            RunNodeResult::NodeDone
        } else {
            let dist_before_short = dist - limits.short_final_dist;
            object.insert(trigger::ByDistance {
                last_observed_pos:  position.horizontal(),
                remaining_distance: dist_before_short,
            });
            RunNodeResult::PendingTrigger
        }
    }

    fn configures_heading(&self, world: &World) -> Option<HorizontalTarget> {
        let runway = world.get::<Runway>(self.runway)?;
        Some(HorizontalTarget::Heading(Heading::from_vec2(runway.landing_length.0)))
    }
}

/// Enforces final approach speed and wait for visual contact with runway.
///
/// Completes when visual contact is established with the runway.
/// Switches to goaround preset if ILS is lost before visual contact is established,
/// e.g. due to ILS interference or low visibility
/// (no visual contact within minimum runway visual range).
///
/// The main goal of this node is to ensure allow ILS-only approach before visual contact;
/// ILS is no longer used after this node completes.
///
/// Must be followed by [`VisualLandingNode`].
#[derive(Clone, Copy)]
pub struct ShortFinalNode {
    /// The runway waypoint entity.
    pub runway:          Entity,
    /// The preset to switch to in case of missed approach.
    pub goaround_preset: Option<Entity>,
}

impl NodeKind for ShortFinalNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        fn classify_navaid(
            runway: Entity,
            navaid: Entity,
            world: &World,
            has_visual: &mut bool,
            has_ils: &mut bool,
        ) {
            let owner = try_log_return!(world.get::<navaid::OwnerWaypoint>(navaid), expect "navaid must have an owner waypoint");
            if owner.0 == runway {
                if world.entity(navaid).contains::<navaid::Visual>() {
                    *has_visual = true;
                }
                if world.entity(navaid).contains::<navaid::LandingAid>() {
                    *has_ils = true;
                }
            }
        }

        let mut object = world.entity_mut(entity);
        if align_runway(&mut object, self.runway, true).is_err() {
            return RunNodeResult::PendingTrigger;
        }

        let &nav::Limits { short_final_speed, .. } = try_log!(
            object.get(),
            expect "Landing aircraft must have nav limits"
            or return RunNodeResult::PendingTrigger
        );

        let mut vel_target = try_log!(
            object.get_mut::<nav::VelocityTarget>(),
            expect "Landing aircraft must have navigation target"
            or return RunNodeResult::PendingTrigger
        );
        vel_target.horiz_speed = short_final_speed;

        let navaids =
            object.get::<navaid::ObjectUsageList>().expect("dependency of VelocityTarget");

        let mut has_visual = false;
        let mut has_ils = false;
        for &navaid in &navaids.0 {
            classify_navaid(self.runway, navaid, object.world(), &mut has_visual, &mut has_ils);
        }

        if has_visual {
            RunNodeResult::NodeDone
        } else if has_ils {
            object.insert(trigger::NavaidChange);
            RunNodeResult::PendingTrigger
        } else {
            RunNodeResult::ReplaceWithPreset(self.goaround_preset)
        }
    }
}

/// Maintains final approach configuration until touchdown.
///
/// Completes when the altitude is below or runway elevation.
/// Switches to goaround preset if:
/// - runway is not clear
/// - runway length is shorter than full deceleration distance to zero speed
/// - unsafe crosswind
/// - intolerable wake
/// - too high (above runway elevation but beyond runway length)
/// - not aligned (beyond runway threshold but not within runway width)
#[derive(Clone, Copy)]
pub struct VisualLandingNode {
    /// The runway waypoint entity.
    pub runway:          Entity,
    /// The preset to switch to in case of missed approach.
    pub goaround_preset: Option<Entity>,
}

impl NodeKind for VisualLandingNode {
    fn run_as_current_node(&self, world: &mut World, object_id: Entity) -> RunNodeResult {
        let mut object = world.entity_mut(object_id);

        let exception =
            match find_landing_state(&object.as_readonly(), &object.world().entity(self.runway)) {
                Err(None) => return RunNodeResult::PendingTrigger,
                Ok(()) => match set_landed(&mut object, self.runway) {
                    Ok(()) => return RunNodeResult::NodeDone,
                    Err(exception) => exception,
                },
                Err(Some(exception)) => exception,
            };

        match exception {
            LandingException::Approaching { remaining_time } => {
                _ = align_runway(&mut object, self.runway, true);

                let virtual_time_now = object.world().resource::<Time<time::Virtual>>().elapsed();
                object.insert(trigger::TimeDelay(virtual_time_now + remaining_time));

                RunNodeResult::PendingTrigger
            }
            LandingException::TooFast => {
                object.world_scope(|world| {
                    message::SendExpiring {
                        source:   object_id,
                        content:  String::from("Going around, too fast"),
                        class:    message::Class::AnomalyInfo,
                        duration: Duration::from_secs(10),
                    }
                    .apply(world);
                });
                RunNodeResult::ReplaceWithPreset(self.goaround_preset)
            }
            LandingException::TooHigh => {
                object.world_scope(|world| {
                    message::SendExpiring {
                        source:   object_id,
                        content:  String::from("Going around, too high"),
                        class:    message::Class::AnomalyInfo,
                        duration: Duration::from_secs(10),
                    }
                    .apply(world);
                });
                RunNodeResult::ReplaceWithPreset(self.goaround_preset)
            }
            LandingException::NotAligned => {
                object.world_scope(|world| {
                    message::SendExpiring {
                        source:   object_id,
                        content:  String::from("Going around, beyond runway width"),
                        class:    message::Class::AnomalyInfo,
                        duration: Duration::from_secs(10),
                    }
                    .apply(world);
                });
                RunNodeResult::ReplaceWithPreset(self.goaround_preset)
            }
            LandingException::TrackDeviate => {
                object.world_scope(|world| {
                    message::SendExpiring {
                        source:   object_id,
                        content:  String::from("Going around, track not parallel to runway"),
                        class:    message::Class::AnomalyInfo,
                        duration: Duration::from_secs(10),
                    }
                    .apply(world);
                });
                RunNodeResult::ReplaceWithPreset(self.goaround_preset)
            }
        }
    }
}

fn set_landed(object: &mut EntityWorldMut, runway_entity: Entity) -> Result<(), LandingException> {
    let &Object { position: object_pos, ground_speed } =
        object.get().expect("checked in find_landing_state");

    let segments = object
        .world()
        .get::<ground::RunwaySegments>(runway_entity)
        .map_or_else(Default::default, |s| &s.0[..]);
    let Some((segment_id, _, alpha_pos, beta_pos)) = segments
        .iter()
        .copied()
        .filter_map(|segment_id| {
            let segment = try_log!(
                object.world().get::<ground::Segment>(segment_id),
                expect "runway must reference valid segment {segment_id:?}"
                or return None
            );
            let alpha_pos = try_log!(
                object.world().get::<ground::Endpoint>(segment.alpha),
                expect "segment must reference valid endpoint {:?}" (segment.alpha)
                or return None
            )
            .position;
            let beta_pos = try_log!(
                object.world().get::<ground::Endpoint>(segment.beta),
                expect "segment must reference valid endpoint {:?}" (segment.beta)
                or return None
            )
            .position;
            Some((segment_id, segment, alpha_pos, beta_pos))
        })
        .find(|&(_, segment, alpha, beta)| {
            segment.contains_pos(alpha, beta, object_pos.horizontal())
        })
    else {
        // TODO send message NotAligned
        return Err(LandingException::NotAligned);
    };

    let ab_speed = ground_speed
        .horizontal()
        .project_onto_dir(Dir2::new((beta_pos - alpha_pos).0).expect("checked in contains_pos"));
    let direction = if ab_speed.is_negative() {
        ground::SegmentDirection::BetaToAlpha
    } else {
        ground::SegmentDirection::AlphaToBeta
    };

    let object_id = object.id();
    object.world_scope(|world| {
        object::SetOnGroundCommand { segment: segment_id, direction }
            .apply(world.entity_mut(object_id));
    });

    Ok(())
}

#[derive(Debug)]
enum LandingException {
    Approaching { remaining_time: Duration },
    TooFast,
    TooHigh,
    NotAligned,
    TrackDeviate,
}

fn find_landing_state(
    object: &EntityRef,
    runway: &EntityRef,
) -> Result<(), Option<LandingException>> {
    let &Object { position: object_position, ground_speed } =
        object.get().expect("entity must be an Object");
    let limits = object.get::<taxi::Limits>().expect("entity must be a navigatable object").clone();
    let &Waypoint { position: runway_position, .. } = try_log!(
        runway.get(), expect "runway must be a waypoint" or return Err(None)
    );
    let &Runway { landing_length, width: runway_width, .. } = try_log!(
        runway.get(), expect "runway must be valid" or return Err(None)
    );
    let runway_condition = try_log!(
        runway.get::<runway::Condition>(), expect "runway must have condition" or return Err(None)
    )
    .clone();

    let runway_dir = try_log!(
        Dir2::new(landing_length.0),
        expect "runway must have nonzero landing length" or return Err(None)
    );

    let projected_speed = ground_speed.horizontal().project_onto_dir(runway_dir);

    let threshold_dist = runway_position - object_position;
    let height = -threshold_dist.vertical();
    let projected_threshold_dist = threshold_dist.horizontal().project_onto_dir(runway_dir);

    if height.is_positive() && projected_threshold_dist.is_positive() {
        let remaining_time = height
            .try_div(-ground_speed.vertical())
            .unwrap_or(Duration::ZERO)
            .min(projected_threshold_dist.try_div(projected_speed).unwrap_or(Duration::ZERO));
        Err(Some(LandingException::Approaching { remaining_time }))
    } else {
        let centerline_dist =
            threshold_dist.horizontal().magnitude_squared() - projected_threshold_dist.squared();
        if centerline_dist > runway_width.squared() {
            return Err(Some(LandingException::NotAligned));
        }

        // if height is non-positive but threshold distance is positive,
        // it basically ditched into terrain before reaching the runway...
        // but for simplicity we just assume it is an extended runway for now.
        // TODO handle aircraft crash
        let remaining_runway_dist = projected_threshold_dist + landing_length.magnitude_exact();

        let required_landing_dist =
            get_required_landing_dist(&limits, &runway_condition, ground_speed.horizontal());

        if remaining_runway_dist < required_landing_dist {
            return Err(Some(if projected_threshold_dist.is_positive() {
                // The entire runway length is insufficient for this speed
                LandingException::TooFast
            } else {
                // The runway length was not considered insufficient until this point
                LandingException::TooHigh
            }));
        }

        let runway_heading = landing_length.heading();
        let track_heading = ground_speed.horizontal().heading();
        let track_deviation = track_heading.closest_distance(runway_heading);
        if track_deviation.abs() > MAX_TRACK_DEVIATION {
            return Err(Some(LandingException::TrackDeviate));
        }

        // TODO check for runway obstacles
        // TODO check wake
        // TODO check crosswind

        Ok(())
    }
}

fn get_required_landing_dist(
    limits: &taxi::Limits,
    runway_condition: &runway::Condition,
    ground_speed: Speed<Vec2>,
) -> Distance<f32> {
    // v^2 = u^2 + 2as => distance = ground_speed.squared() / 2 / deceleration
    let decel = limits.base_braking.0 * runway_condition.friction_factor;
    Distance(ground_speed.magnitude_squared().0 / 2. / decel)
}

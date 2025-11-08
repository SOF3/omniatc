use std::marker::PhantomData;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, EntityCommand, EntityCommands, Query, Res, SystemState};
use bevy::ecs::world::{EntityWorldMut, FromWorld, World};
use bevy::time::{self, Time};
use bevy_mod_config::{AppExt, Config, ConfigFieldFor, Manager, ReadConfig};
use math::{Speed, TurnDirection};
use store::YawTarget;
use wordvec::WordVec;

use super::{SystemSets, nav, route};
use crate::level::object::Object;
use crate::level::route::TaxiStopMode;
use crate::level::waypoint::Waypoint;
use crate::level::{ground, message, object};
use crate::{EntityMutTryLog, EntityTryLog};

pub struct Plug<M>(PhantomData<M>);

impl<M> Default for Plug<M> {
    fn default() -> Self { Self(PhantomData) }
}

impl<M: Manager + Default> Plugin for Plug<M>
where
    Conf: ConfigFieldFor<M>,
{
    fn build(&self, app: &mut App) {
        app.init_config::<M, Conf>("core:instr");
        app.init_resource::<MessageSenderId>();
        app.add_systems(app::Update, dispatch_system.in_set(SystemSets::Communicate));
    }
}

fn dispatch_system(
    conf: ReadConfig<Conf>,
    mut commands: Commands,
    instr_query: Query<
        (Entity, &Instruction, &Recipient, &TransmitDelay, Option<&DispatchAfter>),
        Without<PendingAck>,
    >,
    instr_liveness_query: Query<Has<Instruction>>,
    time: Res<Time<time::Virtual>>,
) {
    let conf = conf.read();

    for (instr_entity, instr, recipient, delay, deps) in instr_query {
        if time.elapsed() < delay.expiry {
            continue;
        }

        let has_alive_deps = deps
            .iter()
            .flat_map(|d| d.dependency.iter())
            .any(|&dep| instr_liveness_query.get(dep) == Ok(true));
        if has_alive_deps {
            continue;
        }

        let mut entity = commands.entity(recipient.0);
        instr.process(&mut entity);
        commands
            .entity(instr_entity)
            .remove::<(Instruction, Recipient, TransmitDelay, PendingAck, DispatchAfter)>()
            .insert(message::Expiry {
                expiry: time.elapsed() + conf.message_duration_after_dispatch,
            });
    }
}

#[derive(Resource)]
struct MessageSenderId(pub Entity);

impl FromWorld for MessageSenderId {
    fn from_world(world: &mut World) -> Self {
        Self(world.spawn(message::Sender { display: "ATC".into() }).id())
    }
}

/// Recipient of an instruction.
///
/// Supported recipient types:
/// - objects
#[derive(Component)]
#[relationship(relationship_target = PendingList)]
pub struct Recipient(pub Entity);

/// Do not dispatch this instruction until after the given instruction entity has been processed.
#[derive(Component)]
pub struct DispatchAfter {
    pub dependency: WordVec<Entity, 1>,
}

/// List of instructions pending processing for a recipient.
#[derive(Component)]
#[relationship_target(relationship = Recipient, linked_spawn)]
pub struct PendingList(Vec<Entity>);

/// Delay before an instruction can be processed,
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct TransmitDelay {
    /// The instruction may be processed after `Time::elapsed()` exceeds this duration.
    pub expiry: Duration,
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct PendingAck;

#[portrait::make]
pub trait Kind {
    fn process(&self, entity: &mut EntityCommands);

    fn format_message(&self, world: &World, object: Entity) -> String;
}

#[derive(Component, derive_more::From)]
#[portrait::derive(Kind with portrait::derive_delegate)]
pub enum Instruction {
    SetHeading(SetHeading),
    SetWaypoint(SetWaypoint),
    SetSpeed(SetSpeed),
    SetAltitude(SetAltitude),
    ClearRoute(ClearRoute),
    SelectRoute(SelectRoute),
    AppendSegment(AppendSegment),
}

pub struct SetHeading {
    pub target: YawTarget,
}

impl Kind for SetHeading {
    fn process(&self, entity: &mut EntityCommands) {
        entity.queue(route::SetStandby);
        entity.remove::<(
            nav::TargetWaypoint,
            nav::TargetGroundDirection,
            nav::TargetAlignment,
            nav::TargetGlide,
            nav::TargetGlideStatus,
        )>();

        let target = self.target;
        entity.queue(move |mut entity: EntityWorldMut| {
            let Some(mut comp) = entity.log_get_mut::<nav::VelocityTarget>() else { return };
            comp.yaw = target;
        });
    }

    fn format_message(&self, _world: &World, _object: Entity) -> String {
        match self.target {
            YawTarget::Heading(heading) => format!("Fly heading {:.0} degrees", heading.degrees()),
            YawTarget::TurnHeading { heading, remaining_crosses: 0, direction } => {
                format!(
                    "Turn {} to heading {:.0} degrees",
                    match direction {
                        TurnDirection::CounterClockwise => "left",
                        TurnDirection::Clockwise => "right",
                    },
                    heading.degrees(),
                )
            }
            YawTarget::TurnHeading { heading, remaining_crosses, direction } => {
                format!(
                    "Turn {} in {remaining_crosses} circles and then stop at heading {:.0} degrees",
                    match direction {
                        TurnDirection::CounterClockwise => "left",
                        TurnDirection::Clockwise => "right",
                    },
                    heading.degrees(),
                )
            }
        }
    }
}

pub struct SetWaypoint {
    pub waypoint: Entity,
}

impl Kind for SetWaypoint {
    fn process(&self, entity: &mut EntityCommands) {
        entity
            .queue(route::SetStandby)
            .remove::<(nav::TargetAlignment, nav::TargetGlide, nav::TargetGlideStatus)>()
            .insert(nav::TargetWaypoint { waypoint_entity: self.waypoint });
    }

    fn format_message(&self, world: &World, _object: Entity) -> String {
        let waypoint = world.entity(self.waypoint);
        let waypoint_name = waypoint.log_get::<Waypoint>().map_or("unknown", |n| n.name.as_str());
        format!("Proceed direct to {waypoint_name}")
    }
}

pub struct SetSpeed {
    pub target: Speed<f32>,
}

impl Kind for SetSpeed {
    fn process(&self, entity: &mut EntityCommands) {
        let target = self.target;
        entity.queue(move |mut entity: EntityWorldMut| {
            let Some(mut comp) = entity.log_get_mut::<nav::VelocityTarget>() else { return };
            comp.horiz_speed = target;
        });
    }

    fn format_message(&self, world: &World, object: Entity) -> String {
        let object = world.entity(object);
        let current_speed =
            object.log_get::<object::Airborne>().map(|a| a.airspeed.magnitude_cmp());
        let verb = match current_speed {
            Some(v) if v > self.target => "Reduce speed to",
            Some(v) if v < self.target => "Increase speed to",
            Some(_) => "Maintain speed",
            None => "Change speed to",
        };
        format!("{verb} {:.0} knots", self.target.into_knots())
    }
}

pub struct SetAltitude {
    pub target: nav::TargetAltitude,
}

impl Kind for SetAltitude {
    fn process(&self, entity: &mut EntityCommands) { entity.insert(self.target.clone()); }

    fn format_message(&self, world: &World, object: Entity) -> String {
        let object = world.entity(object);
        let current_altitude = object.log_get::<Object>().map(|o| o.position.altitude());
        let verb = match current_altitude {
            Some(a) if a > self.target.altitude => "Descend to",
            Some(a) if a < self.target.altitude => "Climb to",
            Some(_) => "Maintain altitude",
            None => "Change altitude to",
        };
        format!("{verb} {} feet", self.target.altitude.amsl().into_feet())
    }
}

pub struct ClearRoute;

impl Kind for ClearRoute {
    fn process(&self, entity: &mut EntityCommands) {
        entity.queue(route::ClearAllNodes).remove::<route::Id>();
    }

    fn format_message(&self, _world: &World, _object: Entity) -> String {
        "Cancel clearance for current route".into()
    }
}

pub struct SelectRoute {
    pub preset: route::Preset,
}

impl Kind for SelectRoute {
    fn process(&self, entity: &mut EntityCommands) {
        entity
            .queue(route::ReplaceNodes(self.preset.nodes.clone()))
            .insert(route::Id(Some(self.preset.id.clone())));
    }

    fn format_message(&self, _: &World, _: Entity) -> String {
        format!("Follow {}", &self.preset.title)
    }
}

pub struct AppendSegment {
    pub clear_existing: bool,
    pub segment:        ground::SegmentLabel,
    pub stop_mode:      TaxiStopMode,
}

impl Kind for AppendSegment {
    fn process(&self, entity: &mut EntityCommands) {
        if self.clear_existing {
            entity.queue(route::ClearAllNodes);
        }

        let label = self.segment.clone();
        let stop_mode = self.stop_mode;
        entity.queue(move |mut entity: EntityWorldMut| {
            entity.insert_if_new(route::Route::default());
            let mut route = entity.get_mut::<route::Route>().expect("just inserted");
            if let Some(route::Node::Taxi(route::TaxiNode { stop: stop_mode, .. })) =
                route.last_mut()
            {
                *stop_mode = TaxiStopMode::Exhaust;
            }
            route.push(route::Node::Taxi(route::TaxiNode {
                label,
                direction: None,
                stop: stop_mode,
            }));
        });
        entity.queue(route::RunCurrentNode);
    }

    fn format_message(&self, world: &World, _object: Entity) -> String {
        let append_message = self.stop_mode.message(self.segment.display_segment_label(world));
        if self.clear_existing {
            format!("Cancel current path, {append_message}")
        } else {
            format!("{append_message} after current path")
        }
    }
}

pub struct SpawnCommand {
    pub object: Entity,
    pub body:   Instruction,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        let transmit_delay = entity.world_scope(|world| {
            let mut state = SystemState::<ReadConfig<Conf>>::new(world);
            state.get(world).read().transmit_delay
        });

        let current_time = entity.world().resource::<Time<time::Virtual>>().elapsed();
        let sender = entity.world().resource::<MessageSenderId>().0;

        let message_content = self.body.format_message(entity.world(), self.object);
        entity.insert((
            self.body,
            Recipient(self.object),
            TransmitDelay { expiry: current_time + transmit_delay },
            message::Message {
                class:   message::Class::Outgoing,
                source:  sender,
                created: current_time,
                content: message_content,
            },
        ));
    }
}

pub trait CommandsExt {
    /// Sends an instruction to an object,
    /// with transmission delay based on configuration.
    fn send_instruction(
        &mut self,
        object: Entity,
        instruction: impl Into<Instruction>,
    ) -> EntityCommands<'_>;
}

impl CommandsExt for Commands<'_, '_> {
    fn send_instruction(
        &mut self,
        object: Entity,
        instruction: impl Into<Instruction>,
    ) -> EntityCommands<'_> {
        let mut entity = self.spawn_empty();
        entity.queue(SpawnCommand { object, body: instruction.into() });
        entity
    }
}

#[derive(Config)]
pub struct Conf {
    /// Delay before an instruction is processed,
    /// simulating the time taken to transmit over radio.
    #[config(default = Duration::ZERO)]
    pub transmit_delay: Duration,

    #[config(default = Duration::from_secs(5))]
    pub message_duration_after_dispatch: Duration,
}

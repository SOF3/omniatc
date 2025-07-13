use bevy::app::{self, App, Plugin};
use bevy::ecs::event::{Event, EventReader};
use bevy::ecs::system::EntityCommands;
use bevy::ecs::world::EntityWorldMut;
use bevy::prelude::{Commands, Entity, IntoScheduleConfigs};
use math::Speed;

use super::{nav, route, SystemSets};
use crate::try_log_return;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<InstructionEvent>();
        app.add_systems(app::Update, process_instr_system.in_set(SystemSets::Communicate));
    }
}

fn process_instr_system(mut commands: Commands, mut events: EventReader<InstructionEvent>) {
    for event in events.read() {
        // TODO validate radio reachability and congestion delay
        event.body.process(commands.entity(event.object));
    }
}

#[portrait::make]
pub trait InstructionKind {
    fn process(&self, commands: EntityCommands);
}

#[derive(Event)]
pub struct InstructionEvent {
    pub object: Entity,
    // pub source: Entity, // TODO radio reachability validation
    pub body:   Instruction,
}

#[derive(derive_more::From)]
#[portrait::derive(InstructionKind with portrait::derive_delegate)]
pub enum Instruction {
    SetHeading(SetHeading),
    SetWaypoint(SetWaypoint),
    SetSpeed(SetSpeed),
    SetAltitude(SetAltitude),
}

pub struct SetHeading {
    pub target: nav::YawTarget,
}

impl InstructionKind for SetHeading {
    fn process(&self, mut entity: EntityCommands) {
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
            let mut comp = try_log_return!(
                entity.get_mut::<nav::VelocityTarget>(),
                expect "cannot override yaw for entity without velocity target"
            );
            comp.yaw = target;
        });
    }
}

pub struct SetWaypoint {
    pub waypoint: Entity,
}

impl InstructionKind for SetWaypoint {
    fn process(&self, mut entity: EntityCommands) {
        entity
            .queue(route::SetStandby)
            .remove::<(nav::TargetAlignment, nav::TargetGlide, nav::TargetGlideStatus)>()
            .insert(nav::TargetWaypoint { waypoint_entity: self.waypoint });
    }
}

pub struct SetSpeed {
    pub target: Speed<f32>,
}

impl InstructionKind for SetSpeed {
    fn process(&self, mut entity: EntityCommands) {
        let target = self.target;
        entity.queue(move |mut entity: EntityWorldMut| {
            let mut comp = try_log_return!(
                entity.get_mut::<nav::VelocityTarget>(),
                expect "cannot override yaw for entity without velocity target"
            );
            comp.horiz_speed = target;
        });
    }
}

pub struct SetAltitude {
    pub target: nav::TargetAltitude,
}

impl InstructionKind for SetAltitude {
    fn process(&self, mut entity: EntityCommands) { entity.insert(self.target.clone()); }
}

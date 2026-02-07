use std::time::Duration;

use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::SystemState;
use bevy::time::{self, Time};
use math::{Heading, Length, Position, Speed};
use store::Score;

use super::*;
use crate::level::ground::{self, SegmentLabel};
use crate::level::instr::{self, Instruction};
use crate::level::object::{self, Object};
use crate::level::score::Stats;
use crate::level::quest;

/// Helper to create a basic app with the condition plugin loaded
fn create_test_app() -> App {
    let mut app = App::new();
    // Add the quest plugin to register messages
    app.add_plugins(quest::Plug);
    app.init_resource::<Time<time::Virtual>>();
    app.init_resource::<Stats>();
    app
}

/// Helper to spawn a quest entity with a condition
fn spawn_quest_with_condition<C: Component>(app: &mut App, condition: C) -> Entity {
    app.world_mut()
        .spawn((
            quest::Quest {
                title: "Test Quest".into(),
                description: "Test Description".into(),
                class: store::QuestClass::Tutorial,
                index: 0,
            },
            quest::Active,
            condition,
        ))
        .id()
}

#[test]
fn test_ui_action_drag() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, UiActionDrag);

    // Verify the condition exists
    assert!(app.world().entity(quest_entity).contains::<UiActionDrag>());

    // Send a camera dragged event
    {
        let mut state = SystemState::<bevy::ecs::message::MessageWriter<quest::UiEvent>>::new(app.world_mut());
        let mut writer = state.get_mut(app.world_mut());
        writer.write_batch(vec![quest::UiEvent::CameraDragged]);
        state.apply(app.world_mut());
    }

    // Update the app to process the event
    app.update();

    // Verify the condition was removed
    assert!(!app.world().entity(quest_entity).contains::<UiActionDrag>());
}

#[test]
fn test_ui_action_zoom() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, UiActionZoom);

    // Verify the condition exists
    assert!(app.world().entity(quest_entity).contains::<UiActionZoom>());

    // Send a camera zoomed event
    {
        let mut state = SystemState::<bevy::ecs::message::MessageWriter<quest::UiEvent>>::new(app.world_mut());
        let mut writer = state.get_mut(app.world_mut());
        writer.write_batch(vec![quest::UiEvent::CameraZoomed]);
        state.apply(app.world_mut());
    }

    // Update the app to process the event
    app.update();

    // Verify the condition was removed
    assert!(!app.world().entity(quest_entity).contains::<UiActionZoom>());
}

#[test]
fn test_ui_action_rotate() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, UiActionRotate);

    // Verify the condition exists
    assert!(app.world().entity(quest_entity).contains::<UiActionRotate>());

    // Send a camera rotated event
    {
        let mut state = SystemState::<bevy::ecs::message::MessageWriter<quest::UiEvent>>::new(app.world_mut());
        let mut writer = state.get_mut(app.world_mut());
        writer.write_batch(vec![quest::UiEvent::CameraRotated]);
        state.apply(app.world_mut());
    }

    // Update the app to process the event
    app.update();

    // Verify the condition was removed
    assert!(!app.world().entity(quest_entity).contains::<UiActionRotate>());
}

#[test]
fn test_reach_altitude() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(
        &mut app,
        ReachAltitude {
            min: Position::from_amsl_feet(2000.0),
            max: Position::from_amsl_feet(4000.0),
        },
    );

    // Spawn an object outside the altitude range
    let object_entity = app
        .world_mut()
        .spawn(Object {
            position: Position::from_origin_nm(0.0, 0.0)
                .with_altitude(Position::from_amsl_feet(5000.0)),
            ground_speed: Speed::ZERO.horizontally(),
        })
        .id();

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<ReachAltitude>());

    // Move object into the altitude range
    app.world_mut()
        .entity_mut(object_entity)
        .get_mut::<Object>()
        .unwrap()
        .position = Position::from_origin_nm(0.0, 0.0)
        .with_altitude(Position::from_amsl_feet(3000.0));

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<ReachAltitude>());
}

#[test]
fn test_reach_speed() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(
        &mut app,
        ReachSpeed {
            min: Speed::from_knots(150.0),
            max: Speed::from_knots(250.0),
        },
    );

    // Spawn an airborne object outside the speed range
    let object_entity = app
        .world_mut()
        .spawn(object::Airborne {
            pressure_alt: Position::from_amsl_feet(3000.0),
            pressure: math::ISA_SEA_LEVEL_PRESSURE,
            oat: math::ISA_SEA_LEVEL_TEMPERATURE,
            airspeed: (Speed::from_knots(100.0) * Heading::NORTH).horizontally(),
            true_airspeed: (Speed::from_knots(100.0) * Heading::NORTH).horizontally(),
        })
        .id();

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<ReachSpeed>());

    // Change object speed to be within range
    app.world_mut()
        .entity_mut(object_entity)
        .get_mut::<object::Airborne>()
        .unwrap()
        .airspeed = (Speed::from_knots(200.0) * Heading::NORTH).horizontally();

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<ReachSpeed>());
}

#[test]
fn test_reach_heading() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(
        &mut app,
        ReachHeading {
            min: Heading::from_degrees(80.0),
            max: Heading::from_degrees(100.0),
        },
    );

    // Spawn an airborne object outside the heading range
    let object_entity = app
        .world_mut()
        .spawn(object::Airborne {
            pressure_alt: Position::from_amsl_feet(3000.0),
            pressure: math::ISA_SEA_LEVEL_PRESSURE,
            oat: math::ISA_SEA_LEVEL_TEMPERATURE,
            airspeed: (Speed::from_knots(200.0) * Heading::NORTH).horizontally(),
            true_airspeed: (Speed::from_knots(200.0) * Heading::NORTH).horizontally(),
        })
        .id();

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<ReachHeading>());

    // Change object heading to be within range
    app.world_mut()
        .entity_mut(object_entity)
        .get_mut::<object::Airborne>()
        .unwrap()
        .airspeed = (Speed::from_knots(200.0) * Heading::from_degrees(90.0)).horizontally();

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<ReachHeading>());
}

#[test]
fn test_reach_segment() {
    let mut app = create_test_app();

    // Create a segment with a specific label
    let segment_entity = app
        .world_mut()
        .spawn(SegmentLabel::Taxiway { name: "A".into() })
        .id();

    let quest_entity = spawn_quest_with_condition(
        &mut app,
        ReachSegment {
            label: SegmentLabel::Taxiway { name: "A".into() },
        },
    );

    // Spawn an object on a different segment
    let other_segment = app
        .world_mut()
        .spawn(SegmentLabel::Taxiway { name: "B".into() })
        .id();

    let object_entity = app
        .world_mut()
        .spawn(object::OnGround {
            segment: other_segment,
            direction: ground::SegmentDirection::AlphaToBeta,
            target_speed: object::OnGroundTargetSpeed::Exact(Speed::ZERO),
        })
        .id();

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<ReachSegment>());

    // Move object to the target segment
    app.world_mut()
        .entity_mut(object_entity)
        .get_mut::<object::OnGround>()
        .unwrap()
        .segment = segment_entity;

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<ReachSegment>());
}

#[test]
fn test_instr_action_direct_waypoint() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, InstrActionDirectWaypoint);

    // Create a waypoint
    let waypoint_entity = app.world_mut().spawn_empty().id();

    // Spawn an instruction entity with SetWaypoint instruction
    app.world_mut().spawn(Instruction::SetWaypoint(instr::SetWaypoint {
        waypoint: waypoint_entity,
    }));

    app.update();

    // Condition should be removed
    assert!(!app
        .world()
        .entity(quest_entity)
        .contains::<InstrActionDirectWaypoint>());
}

#[test]
fn test_instr_action_clear_lineup() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, InstrActionClearLineUp);

    // Spawn an instruction entity with RemoveStandby instruction
    app.world_mut().spawn(Instruction::RemoveStandby(instr::RemoveStandby {
        preset_id: None,
    }));

    app.update();

    // Condition should be removed
    assert!(!app
        .world()
        .entity(quest_entity)
        .contains::<InstrActionClearLineUp>());
}

#[test]
fn test_instr_action_clear_takeoff() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, InstrActionClearTakeoff);

    // Spawn an instruction entity with RemoveStandby instruction
    app.world_mut().spawn(Instruction::RemoveStandby(instr::RemoveStandby {
        preset_id: None,
    }));

    app.update();

    // Condition should be removed
    assert!(!app
        .world()
        .entity(quest_entity)
        .contains::<InstrActionClearTakeoff>());
}

#[test]
fn test_instr_action_follow_route() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, InstrActionFollowRoute);

    // Spawn an instruction entity with SelectRoute instruction
    app.world_mut().spawn(Instruction::SelectRoute(instr::SelectRoute {
        preset: crate::level::route::Preset {
            id: "test-route".into(),
            title: "Test Route".into(),
            nodes: vec![],
        },
    }));

    app.update();

    // Condition should be removed
    assert!(!app
        .world()
        .entity(quest_entity)
        .contains::<InstrActionFollowRoute>());
}

#[test]
fn test_min_landing() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, MinLanding { landings: 3 });

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<MinLanding>());

    // Update the stats to reach the minimum
    app.world_mut().resource_mut::<Stats>().num_runway_arrivals = 3;

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<MinLanding>());
}

#[test]
fn test_min_parking() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, MinParking { parkings: 5 });

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<MinParking>());

    // Update the stats to reach the minimum
    app.world_mut().resource_mut::<Stats>().num_apron_arrivals = 5;

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<MinParking>());
}

#[test]
fn test_min_departure() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, MinDeparture { departures: 2 });

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<MinDeparture>());

    // Update the stats to reach the minimum
    app.world_mut().resource_mut::<Stats>().num_departures = 2;

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<MinDeparture>());
}

#[test]
fn test_min_score() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, MinScore { score: Score(1000) });

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<MinScore>());

    // Update the stats to reach the minimum score
    app.world_mut().resource_mut::<Stats>().total = Score(1000);

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<MinScore>());
}

#[test]
fn test_max_conflicts() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(&mut app, MaxConflicts { conflicts: 2 });

    // Initially there are no conflicts, so the condition should complete
    app.update();

    // Condition should be removed immediately since conflicts are 0
    assert!(!app.world().entity(quest_entity).contains::<MaxConflicts>());
}

#[test]
fn test_max_conflicts_fails() {
    let mut app = create_test_app();

    // Set initial conflicts above the max
    app.world_mut().resource_mut::<Stats>().num_conflicts = 5;

    let quest_entity = spawn_quest_with_condition(&mut app, MaxConflicts { conflicts: 2 });

    app.update();

    // Condition should still exist because we have too many conflicts
    assert!(app.world().entity(quest_entity).contains::<MaxConflicts>());
}

#[test]
fn test_time_elapsed() {
    let mut app = create_test_app();

    let quest_entity = spawn_quest_with_condition(
        &mut app,
        TimeElapsed {
            time: Duration::from_secs(10),
        },
    );

    app.update();

    // Condition should still exist (no time has passed)
    assert!(app.world().entity(quest_entity).contains::<TimeElapsed>());

    // Advance time by 5 seconds (not enough)
    app.world_mut()
        .resource_mut::<Time<time::Virtual>>()
        .advance_by(Duration::from_secs(5));

    app.update();

    // Condition should still exist
    assert!(app.world().entity(quest_entity).contains::<TimeElapsed>());

    // Advance time by another 5 seconds (total 10 seconds)
    app.world_mut()
        .resource_mut::<Time<time::Virtual>>()
        .advance_by(Duration::from_secs(5));

    app.update();

    // Condition should be removed
    assert!(!app.world().entity(quest_entity).contains::<TimeElapsed>());
}

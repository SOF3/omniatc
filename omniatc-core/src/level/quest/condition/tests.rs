use std::time::Duration;

use bevy::app::App;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::time::{self, Time};
use math::{Heading, Position, Speed};
use store::Score;

use super::*;
use crate::level::ground::{self, SegmentLabel};
use crate::level::instr::{self, Instruction};
use crate::level::object::{self, Object};
use crate::level::score::Stats;
use crate::level::{quest, route};

fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(quest::Plug);
    app.init_resource::<Time<time::Virtual>>();
    app.init_resource::<Stats>();
    app
}

fn quest_with_condition<C: Component>(condition: C) -> impl Bundle {
    (
        quest::Quest {
            title:       "Test Quest".into(),
            description: "Test Description".into(),
            class:       store::QuestClass::Tutorial,
            index:       0,
        },
        quest::Active,
        condition,
    )
}

fn test_ui_action<C: Component + Default>(condition: C, event: quest::UiEvent) {
    let mut app = create_test_app();
    let quest_entity = app.world_mut().spawn(quest_with_condition(condition)).id();

    assert!(app.world().entity(quest_entity).contains::<C>());

    app.world_mut().write_message(event);
    app.update();

    assert!(!app.world().entity(quest_entity).contains::<C>());
}

#[test]
fn test_ui_action_drag() { test_ui_action(UiActionDrag, quest::UiEvent::CameraDragged); }

#[test]
fn test_ui_action_zoom() { test_ui_action(UiActionZoom, quest::UiEvent::CameraZoomed); }

#[test]
fn test_ui_action_rotate() { test_ui_action(UiActionRotate, quest::UiEvent::CameraRotated); }

#[test]
fn test_reach_altitude() {
    let mut app = create_test_app();

    let quest_entity = app
        .world_mut()
        .spawn(quest_with_condition(ReachAltitude {
            min: Position::from_amsl_feet(2000.0),
            max: Position::from_amsl_feet(4000.0),
        }))
        .id();

    let object_entity = app
        .world_mut()
        .spawn(Object {
            position:     Position::from_origin_nm(0.0, 0.0)
                .with_altitude(Position::from_amsl_feet(5000.0)),
            ground_speed: Speed::ZERO.horizontally(),
        })
        .id();

    app.update();

    assert!(
        app.world().entity(quest_entity).contains::<ReachAltitude>(),
        "object is not within 2000ft..4000ft"
    );

    app.world_mut().entity_mut(object_entity).get_mut::<Object>().unwrap().position =
        Position::from_origin_nm(0.0, 0.0).with_altitude(Position::from_amsl_feet(3000.0));

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<ReachAltitude>());
}

#[test]
fn test_reach_speed() {
    let mut app = create_test_app();

    let quest_entity = app
        .world_mut()
        .spawn(quest_with_condition(ReachSpeed {
            min: Speed::from_knots(150.0),
            max: Speed::from_knots(250.0),
        }))
        .id();

    let object_entity = app
        .world_mut()
        .spawn(object::Airborne {
            pressure_alt:  Position::from_amsl_feet(3000.0),
            pressure:      math::ISA_SEA_LEVEL_PRESSURE,
            oat:           math::ISA_SEA_LEVEL_TEMPERATURE,
            airspeed:      (Speed::from_knots(100.0) * Heading::NORTH).horizontally(),
            true_airspeed: (Speed::from_knots(100.0) * Heading::NORTH).horizontally(),
        })
        .id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<ReachSpeed>());

    app.world_mut().entity_mut(object_entity).get_mut::<object::Airborne>().unwrap().airspeed =
        (Speed::from_knots(200.0) * Heading::NORTH).horizontally();

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<ReachSpeed>());
}

#[test]
fn test_reach_heading() {
    let mut app = create_test_app();

    let quest_entity = app
        .world_mut()
        .spawn(quest_with_condition(ReachHeading {
            min: Heading::from_degrees(350.0),
            max: Heading::from_degrees(10.0),
        }))
        .id();

    let object_entity = app
        .world_mut()
        .spawn(object::Airborne {
            pressure_alt:  Position::from_amsl_feet(3000.0),
            pressure:      math::ISA_SEA_LEVEL_PRESSURE,
            oat:           math::ISA_SEA_LEVEL_TEMPERATURE,
            airspeed:      (Speed::from_knots(200.0) * Heading::SOUTH).horizontally(),
            true_airspeed: (Speed::from_knots(200.0) * Heading::SOUTH).horizontally(),
        })
        .id();

    app.update();

    assert!(
        app.world().entity(quest_entity).contains::<ReachHeading>(),
        "heading 180° is not within 350°..10°"
    );

    app.world_mut().entity_mut(object_entity).get_mut::<object::Airborne>().unwrap().airspeed =
        (Speed::from_knots(200.0) * Heading::NORTH).horizontally();

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<ReachHeading>());
}

#[test]
fn test_reach_segment() {
    let mut app = create_test_app();

    let [desired_segment, initial_segment] = ["A", "B"]
        .map(|name| app.world_mut().spawn(SegmentLabel::Taxiway { name: name.into() }).id());

    let quest_entity = app
        .world_mut()
        .spawn(quest_with_condition(ReachSegment {
            label: SegmentLabel::Taxiway { name: "A".into() },
        }))
        .id();

    let object_entity = app
        .world_mut()
        .spawn(object::OnGround {
            segment:      initial_segment,
            direction:    ground::SegmentDirection::AlphaToBeta,
            target_speed: object::OnGroundTargetSpeed::Exact(Speed::ZERO),
        })
        .id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<ReachSegment>());

    app.world_mut().entity_mut(object_entity).get_mut::<object::OnGround>().unwrap().segment =
        desired_segment;

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<ReachSegment>());
}

#[test]
fn test_instr_action_direct_waypoint() {
    let mut app = create_test_app();

    let quest_entity = app.world_mut().spawn(quest_with_condition(InstrActionDirectWaypoint)).id();

    let waypoint_entity = app.world_mut().spawn_empty().id();

    app.world_mut()
        .spawn(Instruction::SetWaypoint(instr::SetWaypoint { waypoint: waypoint_entity }));

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<InstrActionDirectWaypoint>());
}

#[test]
fn test_instr_action_clear_lineup() {
    let mut app = create_test_app();

    let quest_entity = app.world_mut().spawn(quest_with_condition(InstrActionClearLineUp)).id();

    // TODO: This will be better tested when we have explicit standby removal instructions
    app.world_mut().spawn(Instruction::RemoveStandby(instr::RemoveStandby { skip_id: None }));

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<InstrActionClearLineUp>());
}

#[test]
fn test_instr_action_clear_takeoff() {
    let mut app = create_test_app();

    let quest_entity = app.world_mut().spawn(quest_with_condition(InstrActionClearTakeoff)).id();

    // TODO: This will be better tested when we have explicit standby removal instructions
    app.world_mut().spawn(Instruction::RemoveStandby(instr::RemoveStandby { skip_id: None }));

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<InstrActionClearTakeoff>());
}

#[test]
fn test_instr_action_follow_route() {
    let mut app = create_test_app();

    let quest_entity = app.world_mut().spawn(quest_with_condition(InstrActionFollowRoute)).id();

    app.world_mut().spawn(Instruction::SelectRoute(instr::SelectRoute {
        preset: route::Preset {
            id:    "test-route".into(),
            title: "Test Route".into(),
            nodes: Vec::new(),
        },
    }));

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<InstrActionFollowRoute>());
}

#[test]
fn test_min_landing() {
    let mut app = create_test_app();

    let quest_entity = app.world_mut().spawn(quest_with_condition(MinLanding { landings: 3 })).id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<MinLanding>());

    app.world_mut().resource_mut::<Stats>().num_runway_arrivals = 3;

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<MinLanding>());
}

#[test]
fn test_min_parking() {
    let mut app = create_test_app();

    let quest_entity = app.world_mut().spawn(quest_with_condition(MinParking { parkings: 5 })).id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<MinParking>());

    app.world_mut().resource_mut::<Stats>().num_apron_arrivals = 5;

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<MinParking>());
}

#[test]
fn test_min_departure() {
    let mut app = create_test_app();

    let quest_entity =
        app.world_mut().spawn(quest_with_condition(MinDeparture { departures: 2 })).id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<MinDeparture>());

    app.world_mut().resource_mut::<Stats>().num_departures = 2;

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<MinDeparture>());
}

#[test]
fn test_min_score() {
    let mut app = create_test_app();

    let quest_entity =
        app.world_mut().spawn(quest_with_condition(MinScore { score: Score(1000) })).id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<MinScore>());

    app.world_mut().resource_mut::<Stats>().total = Score(1000);

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<MinScore>());
}

#[test]
fn test_max_conflicts() {
    let mut app = create_test_app();

    let quest_entity =
        app.world_mut().spawn(quest_with_condition(MaxConflicts { conflicts: 2 })).id();

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<MaxConflicts>());
}

#[test]
fn test_max_conflicts_fails() {
    let mut app = create_test_app();

    app.world_mut().resource_mut::<Stats>().num_conflicts = 5;

    let quest_entity =
        app.world_mut().spawn(quest_with_condition(MaxConflicts { conflicts: 2 })).id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<MaxConflicts>());
}

#[test]
fn test_time_elapsed() {
    let mut app = create_test_app();

    let quest_entity = app
        .world_mut()
        .spawn(quest_with_condition(TimeElapsed { time: Duration::from_secs(10) }))
        .id();

    app.update();

    assert!(app.world().entity(quest_entity).contains::<TimeElapsed>());

    app.world_mut().resource_mut::<Time<time::Virtual>>().advance_by(Duration::from_secs(5));

    app.update();

    assert!(app.world().entity(quest_entity).contains::<TimeElapsed>());

    app.world_mut().resource_mut::<Time<time::Virtual>>().advance_by(Duration::from_secs(5));

    app.update();

    assert!(!app.world().entity(quest_entity).contains::<TimeElapsed>());
}

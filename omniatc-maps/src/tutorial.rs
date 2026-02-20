use math::{Accel, AngularSpeed, Heading, Length, Position, Speed};
use store::Score;

use crate::{common_types, demo};

fn quests(waypoints: Vec<store::NamedWaypointRef>) -> impl Into<Vec<store::Quest>> {
    [
        store::Quest {
            id:               "tutorial/drag".into(),
            title:            "Tutorial: Camera (1/3)".into(),
            description:      "Right-click the radar view and drag to move the camera.".into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     [].into(),
            conditions:       [store::UiQuestCompletionCondition::CameraDrag.into()].into(),
            ui_highlight:     [store::HighlightableUiElement::RadarView].into(),
            completion_hooks: [].into(),
        },
        store::Quest {
            id:               "tutorial/zoom".into(),
            title:            "Tutorial: Camera (2/3)".into(),
            description:      "Scroll on the radar view up and down to zoom in and out.".into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     ["tutorial/drag".into()].into(),
            conditions:       [store::UiQuestCompletionCondition::CameraZoom.into()].into(),
            ui_highlight:     [
                store::HighlightableUiElement::RadarView,
                store::HighlightableUiElement::SetCameraZoom,
            ]
            .into(),
            completion_hooks: [].into(),
        },
        store::Quest {
            id:               "tutorial/rotate".into(),
            title:            "Tutorial: Camera (3/3)".into(),
            description:      concat!(
                "Scroll on the radar view left and right, ",
                "or use the slider in the Level menu to rotate.",
            )
            .into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     ["tutorial/zoom".into()].into(),
            conditions:       [store::UiQuestCompletionCondition::CameraRotate.into()].into(),
            ui_highlight:     [
                store::HighlightableUiElement::RadarView,
                store::HighlightableUiElement::SetCameraRotation,
            ]
            .into(),
            completion_hooks: [store::QuestCompletionHook::SpawnObject {
                object: Box::new(store::Object::Plane(store::Plane {
                    aircraft:    store::BaseAircraft {
                        name:             "ABC123".into(),
                        dest:             store::Destination::Landing { aerodrome: "MAIN".into() },
                        completion_score: Score(10),
                        position:         Position::from_origin_nm(-5.0, -5.0),
                        altitude:         Position::from_amsl_feet(8000.0),
                        ground_speed:     Speed::from_knots(289.0),
                        ground_dir:       Heading::EAST,
                        vert_rate:        Speed::ZERO,
                    },
                    control:     store::PlaneControl {
                        heading:     Heading::EAST,
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    object_type: store::ObjectTypeRef("A359".into()),
                    taxi_limits: common_types::a359_taxi_limits(),
                    nav_limits:  common_types::a359_nav_limits(),
                    nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                        yaw:              store::YawTarget::Heading(Heading::EAST),
                        horiz_ias:        Some(Speed::from_knots(280.0)),
                        vert_rate:        Speed::from_fpm(0.0),
                        expedite:         false,
                        target_altitude:  None,
                        target_glide:     None,
                        target_waypoint:  None,
                        target_alignment: None,
                    })),
                    route:       store::Route { id: None, nodes: Vec::new() },
                })),
            }]
            .into(),
        },
        store::Quest {
            id:               "tutorial/focus".into(),
            title:            "Tutorial: Aircraft control (1/5)".into(),
            description:      concat!(
                "An aircraft has just entered our airspace! ",
                "Click on it in the radar view or in the Vehicles table to view details.",
            )
            .into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     ["tutorial/rotate".into()].into(),
            conditions:       [store::UiQuestCompletionCondition::ObjectSelect.into()].into(),
            ui_highlight:     [store::HighlightableUiElement::ObjectSelect].into(),
            completion_hooks: [].into(),
        },
        store::Quest {
            id:               "tutorial/altitude".into(),
            title:            "Tutorial: Aircraft control (2/5)".into(),
            description:      concat!(
                "The aircraft is currently at 8000 feet. ",
                "Let's prepare it for landing by descending to 6000 feet.\n",
                r#"Drag the altitude slider and click "Send" "#,
                "to instruct the pilot to change altitude. ",
                "You may also use up/down arrow keys and press Enter.\n",
                "It will take a few moments for the aircraft to change altitude, ",
                "so don't worry if it doesn't happen immediately!\n",
                r#"To speed things up a bit, you can use the "Game speed" slider on the left, "#,
                "or hold Shift-Space to fast-forward.",
            )
            .into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     ["tutorial/focus".into()].into(),
            conditions:       [store::ObjectControlQuestCompletionCondition::ReachAltitude(
                store::Range {
                    min: Position::from_amsl_feet(5950.0),
                    max: Position::from_amsl_feet(6050.0),
                },
            )
            .into()]
            .into(),
            ui_highlight:     [store::HighlightableUiElement::SetAltitude].into(),
            completion_hooks: [].into(),
        },
        store::Quest {
            id:               "tutorial/speed".into(),
            title:            "Tutorial: Aircraft control (3/5)".into(),
            description:      concat!(
                "The plane is too fast to land right now. Let's slow it down to 230 knots.\n",
                r#"Drag the speed slider on the right, or press ","/"." to adjust the speed. "#,
                r#"Remember to click "Send" or press Enter to send the instruction!"#,
            )
            .into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     ["tutorial/focus".into()].into(),
            conditions:       [store::ObjectControlQuestCompletionCondition::ReachSpeed(
                store::Range { min: Speed::from_knots(225.0), max: Speed::from_knots(235.0) },
            )
            .into()]
            .into(),
            ui_highlight:     [store::HighlightableUiElement::SetSpeed].into(),
            completion_hooks: [].into(),
        },
        store::Quest {
            id:               "tutorial/heading".into(),
            title:            "Tutorial: Aircraft control (4/5)".into(),
            description:      concat!(
                "The plane is currently above the aerodrome east of the runways. ",
                "The current wind direction is from the southeast, ",
                "so arrivals will land from the north of the runway. ",
                "To prepare for landing, we will guide the plane to enter the downwind leg ",
                "by flying parallel to the runway but in the opposite direction of landing.\n",
                "Let's turn it to the downwind leg by turning left to the north. ",
                "Point your cursor north of the plane, then click \"v\" on your keyboard.\n",
                "Alternatively, use the left/right arrow keys to adjust the heading. ",
                "Don't forget to click \"Send\" or press Enter to send the instruction.\n",
                "Note that the final heading will be affected by wind, ",
                "so I recommend giving a heading of 005\u{b0} to compensate for it."
            )
            .into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     ["tutorial/altitude".into(), "tutorial/speed".into()].into(),
            conditions:       [store::ObjectControlQuestCompletionCondition::ReachHeading(
                store::Range {
                    min: Heading::from_degrees(355.0),
                    max: Heading::from_degrees(15.0),
                },
            )
            .into()]
            .into(),
            ui_highlight:     [store::HighlightableUiElement::SetHeading].into(),
            completion_hooks: waypoints
                .into_iter()
                .map(|waypoint| store::QuestCompletionHook::RevealWaypoint { waypoint })
                .collect(),
        },
    ]
}

#[must_use]
pub fn file() -> store::File {
    let mut demo_level = demo::level();
    let mut waypoints = Vec::new();
    for waypoint in &mut demo_level.waypoints {
        waypoint.hidden = true;
        waypoints.push(store::NamedWaypointRef(waypoint.name.clone()));
    }
    demo_level.spawn_sets = [].into();
    demo_level.spawn_trigger = store::SpawnTrigger::Disabled;

    store::File {
        meta:    store::Meta {
            id:          "omniatc.tutorial".into(),
            title:       "Tutorial".into(),
            description: "Tutorial map".into(),
            authors:     ["omniatc".into()].into(),
            tags:        [
                ("region", "fictional"),
                ("source", "builtin"),
                ("type", "scenario"),
                ("tutorial", "true"),
            ]
            .into_iter()
            .map(|(k, v)| (String::from(k), String::from(v)))
            .collect(),
        },
        level:   demo_level,
        ui:      store::Ui {
            camera: store::Camera::TwoDimension(store::Camera2d {
                center:       Position::from_origin_nm(0.0, 0.0),
                up:           Heading::NORTH,
                scale_axis:   store::AxisDirection::X,
                scale_length: Length::from_nm(100.0),
            }),
        },
        stats:   store::Stats::default(),
        quests:  store::QuestTree { quests: quests(waypoints).into() },
        objects: [].into(),
    }
}

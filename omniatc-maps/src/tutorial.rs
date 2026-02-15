use math::{Accel, AngularSpeed, Heading, Length, Position, Speed};
use store::Score;

use crate::{common_types, demo};

fn quests() -> impl Into<Vec<store::Quest>> {
    [
        store::Quest {
            id:               "tutorial/drag".into(),
            title:            "Tutorial: Camera (1/3)".into(),
            description:      "Right-click the radar view and drag to move the camera.".into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     [].into(),
            conditions:       [store::CameraQuestCompletionCondition::Drag.into()].into(),
            ui_highlight:     [store::HighlightableUiElement::RadarView].into(),
            completion_hooks: [].into(),
        },
        store::Quest {
            id:               "tutorial/zoom".into(),
            title:            "Tutorial: Camera (2/3)".into(),
            description:      "Scroll on the radar view up and down to zoom in and out.".into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     ["tutorial/drag".into()].into(),
            conditions:       [store::CameraQuestCompletionCondition::Zoom.into()].into(),
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
            conditions:       [store::CameraQuestCompletionCondition::Rotate.into()].into(),
            ui_highlight:     [
                store::HighlightableUiElement::RadarView,
                store::HighlightableUiElement::SetCameraRotation,
            ]
            .into(),
            completion_hooks: [store::QuestCompletionHook::SpawnObject {
                object: store::Object::Plane(store::Plane {
                    aircraft:    store::BaseAircraft {
                        name:             "ABC123".into(),
                        dest:             store::Destination::Landing { aerodrome: "MAIN".into() },
                        completion_score: Score(10),
                        position:         Position::from_origin_nm(2., -14.),
                        altitude:         Position::from_amsl_feet(12000.),
                        ground_speed:     Speed::from_knots(280.),
                        ground_dir:       Heading::from_degrees(250.),
                        vert_rate:        Speed::ZERO,
                    },
                    control:     store::PlaneControl {
                        heading:     Heading::from_degrees(80.),
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    object_type: store::ObjectTypeRef("A359".into()),
                    taxi_limits: common_types::a359_taxi_limits(),
                    nav_limits:  common_types::a359_nav_limits(),
                    nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                        yaw:              store::YawTarget::Heading(Heading::from_degrees(80.)),
                        horiz_speed:      Speed::from_knots(280.),
                        vert_rate:        Speed::from_fpm(0.),
                        expedite:         false,
                        target_altitude:  None,
                        target_glide:     None,
                        target_waypoint:  None,
                        target_alignment: None,
                    })),
                    route:       store::Route {
                        id:    Some("DWIND18L".into()),
                        nodes: demo::route_dwind_18l(),
                    },
                }),
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
            dependencies:     [].into(),
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
            id:               "tutorial/altitude".into(),
            title:            "Tutorial: Aircraft control (2/5)".into(),
            description:      concat!(
                r#"Drag the altitude slider and click "Send" "#,
                "to instruct the pilot to change altitude. ",
                "You may also use up/down arrow keys and press Enter.\n",
                "Let's prepare it for landing by descending to 6000 feet.",
            )
            .into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     [].into(),
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
                r#"Drag the speed slider or press ","/"." to adjust the speed. "#,
                "Let's slow down the aircraft to 250 knots.",
            )
            .into(),
            class:            store::QuestClass::Tutorial,
            dependencies:     [].into(),
            conditions:       [store::ObjectControlQuestCompletionCondition::ReachSpeed(
                store::Range { min: Speed::from_knots(245.0), max: Speed::from_knots(255.0) },
            )
            .into()]
            .into(),
            ui_highlight:     [].into(),
            completion_hooks: [].into(),
        },
    ]
}

#[must_use]
pub fn file() -> store::File {
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
        level:   store::Level {
            spawn_sets: [].into(),
            spawn_trigger: store::SpawnTrigger::Disabled,
            ..demo::level()
        },
        ui:      store::Ui {
            camera: store::Camera::TwoDimension(store::Camera2d {
                center:       Position::from_origin_nm(0., 0.),
                up:           Heading::NORTH,
                scale_axis:   store::AxisDirection::X,
                scale_length: Length::from_nm(100.),
            }),
        },
        stats:   store::Stats::default(),
        quests:  store::QuestTree { quests: quests().into() },
        objects: [].into(),
    }
}

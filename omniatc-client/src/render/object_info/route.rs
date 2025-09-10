use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, Query, Res, SystemParam};
use bevy_egui::egui;
use itertools::Itertools;
use omniatc::QueryTryLog;
use omniatc::level::aerodrome::Aerodrome;
use omniatc::level::route::{self, Route};
use omniatc::level::runway::RunwayOf;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{ground, nav, taxi};
use store::WaypointProximity;

use super::{Writer, dir};
use crate::input;
use crate::util::new_type_id;

#[derive(QueryData)]
pub struct ObjectQuery {
    route:           Option<&'static Route>,
    route_id:        Option<&'static route::Id>,
    target_waypoint: Option<&'static nav::TargetWaypoint>,
    taxi_target:     Option<&'static taxi::Target>,
    entity:          Entity,
}

#[derive(SystemParam)]
pub struct WriteRouteParams<'w, 's> {
    waypoint_query:         Query<'w, 's, &'static Waypoint>,
    waypoint_presets_query: Query<'w, 's, &'static route::WaypointPresetList>,
    preset_query:           Query<'w, 's, &'static route::Preset>,
    runway_query:           Query<'w, 's, (&'static Waypoint, &'static RunwayOf)>,
    aerodrome_query:        Query<'w, 's, &'static Aerodrome>,
    segment_query:          Query<'w, 's, &'static ground::SegmentLabel>,
    commands:               Commands<'w, 's>,
    hotkeys:                Res<'w, input::Hotkeys>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteRouteParams<'w, 's>;

    fn title() -> &'static str { "Route" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        if let Some(target) = this.target_waypoint
            && let Ok(presets) = params.waypoint_presets_query.get(target.waypoint_entity)
        {
            write_route_options(
                ui,
                &params.preset_query,
                &mut params.commands,
                this.entity,
                presets,
                this.route_id.and_then(|id| id.0.as_deref()),
                &params.hotkeys,
            );
        }

        if let Some(target) = this.taxi_target {
            write_taxi_target(ui, target, params);
        }

        if let Some(route) = this.route {
            for node in route.iter() {
                write_route_node(ui, node, this.entity, params);
            }
        }
    }
}

fn write_route_options(
    ui: &mut egui::Ui,
    preset_query: &Query<&route::Preset>,
    commands: &mut Commands,
    object: Entity,
    presets: &route::WaypointPresetList,
    current_route_id: Option<&str>,
    hotkeys: &input::Hotkeys,
) {
    #[derive(Clone, Copy, PartialEq)]
    enum Selection {
        None,
        Available(usize),
        Retain,
    }

    let presets: Vec<_> =
        presets.iter().filter_map(|entity| preset_query.log_get(entity)).collect();

    if current_route_id.is_none() && presets.is_empty() {
        return;
    }

    let current_index =
        current_route_id.and_then(|curr| presets.iter().position(|preset| preset.id == curr));
    let current_selection = match (current_index, current_route_id) {
        (None, Some(_)) => Selection::Retain,
        (None, None) => Selection::None,
        (Some(value), _) => Selection::Available(value),
    };

    let mut selection = current_selection;
    if hotkeys.next_route {
        selection = match selection {
            Selection::None => Selection::Available(0),
            Selection::Available(n) => Selection::Available(n + 1),
            Selection::Retain => Selection::None,
        };
        if let Selection::Available(n) = selection
            && presets.get(n).is_none()
        {
            selection = Selection::None;
        }
    }

    ui.horizontal(|ui| {
        ui.label("Route");

        egui::ComboBox::from_label("").selected_text(current_route_id.unwrap_or("None")).show_ui(
            ui,
            |ui| {
                ui.selectable_value(&mut selection, Selection::None, "None");
                if current_selection == Selection::Retain {
                    ui.selectable_value(
                        &mut selection,
                        Selection::Retain,
                        current_route_id.unwrap(),
                    );
                }
                for (i, preset) in presets.iter().enumerate() {
                    ui.selectable_value(&mut selection, Selection::Available(i), &preset.title);
                }
            },
        );
        if selection != current_selection {
            match selection {
                Selection::None => {
                    commands.entity(object).queue(route::ClearAllNodes).remove::<route::Id>();
                }
                Selection::Available(index) => {
                    let new_preset = presets[index];
                    commands
                        .entity(object)
                        .queue(route::ReplaceNodes(new_preset.nodes.clone()))
                        .insert(route::Id(Some(new_preset.id.clone())));
                }
                Selection::Retain => {}
            }
        }
    });
}

fn write_taxi_target(ui: &mut egui::Ui, target: &taxi::Target, params: &WriteRouteParams) {
    match target.action {
        taxi::TargetAction::HoldShort => {
            ui.label("Hold short at the next intersection");
        }
        taxi::TargetAction::Taxi { ref options } => {
            let label_strs = options
                .iter()
                .filter_map(|&segment| {
                    let label = params.segment_query.log_get(segment)?;
                    Some(dir::display_segment_label(label, &params.waypoint_query))
                })
                .join(" or ");
            ui.label(format!("Turn to {label_strs}"));
        }
    }
}

fn write_route_node(
    ui: &mut egui::Ui,
    node: &route::Node,
    entity: Entity,
    params: &mut WriteRouteParams,
) {
    match node {
        route::Node::Standby(_) => {
            if ui.button("Resume route").clicked() {
                params.commands.entity(entity).queue(route::NextNode);
            }
        }
        route::Node::DirectWaypoint(node) => {
            let Some(waypoint) = params.waypoint_query.log_get(node.waypoint) else { return };
            match node.proximity {
                WaypointProximity::FlyBy => ui.label(format!("Fly by {}", &waypoint.name)),
                WaypointProximity::FlyOver => ui.label(format!("Fly over {}", &waypoint.name)),
            };

            if let Some(altitude) = node.altitude {
                ui.indent(new_type_id!(), |ui| {
                    ui.label(format!("Pass at altitude {:.0} ft", altitude.amsl().into_feet()));
                });
            }
        }
        route::Node::SetAirSpeed(node) => {
            ui.label(format!("Set speed to {:.0} kt", node.speed.into_knots()));
            if let Some(error) = node.error {
                ui.indent(new_type_id!(), |ui| {
                    ui.label(format!("Maintain until \u{b1}{:.0} kt", error.into_knots()));
                });
            }
        }
        route::Node::StartSetAltitude(node) => {
            let expedite = if node.expedite { " (expedite)" } else { "" };
            ui.label(format!(
                "Start approaching altitude {:.0} ft{expedite}",
                node.altitude.amsl().into_feet()
            ));
            if let Some(error) = node.error {
                ui.indent(new_type_id!(), |ui| {
                    ui.label(format!("Maintain until \u{b1}{:.0} ft", error.into_feet()));
                });
            }
        }
        route::Node::AlignRunway(node) => {
            let Some((waypoint, &RunwayOf(aerodrome_id))) =
                params.runway_query.log_get(node.runway)
            else {
                return;
            };
            let Some(aerodrome) = params.aerodrome_query.log_get(aerodrome_id) else { return };
            ui.label(format!("Align with ILS {} of {}", &waypoint.name, &aerodrome.name));
        }
        route::Node::ShortFinal(node) => {
            let Some((waypoint, &RunwayOf(aerodrome_id))) =
                params.runway_query.log_get(node.runway)
            else {
                return;
            };
            let Some(aerodrome) = params.aerodrome_query.log_get(aerodrome_id) else { return };
            ui.label(format!("Short final to ILS {} of {}", &waypoint.name, &aerodrome.name));
        }
        route::Node::VisualLanding(node) => {
            let Some((waypoint, &RunwayOf(aerodrome_id))) =
                params.runway_query.log_get(node.runway)
            else {
                return;
            };
            let Some(aerodrome) = params.aerodrome_query.log_get(aerodrome_id) else { return };
            ui.label(format!(
                "Visual short final to runway {} of {}",
                &waypoint.name, &aerodrome.name
            ));
        }
        route::Node::Taxi(node) => {
            if node.hold_short {
                ui.label(format!(
                    "Hold short at {}",
                    dir::display_segment_label(&node.label, &params.waypoint_query)
                ));
            } else {
                ui.label(format!(
                    "Taxi via {}",
                    dir::display_segment_label(&node.label, &params.waypoint_query)
                ));
            }
        }
    }
}

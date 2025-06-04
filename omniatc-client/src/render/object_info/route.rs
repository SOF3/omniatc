use std::any::TypeId;

use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, Query, SystemParam};
use bevy_egui::egui;
use omniatc::level::aerodrome::Aerodrome;
use omniatc::level::nav;
use omniatc::level::route::{self, Route};
use omniatc::level::runway::Runway;
use omniatc::level::waypoint::Waypoint;
use omniatc::{try_log, try_log_return};

use super::Writer;

#[derive(QueryData)]
pub struct ObjectQuery {
    route:           Option<&'static Route>,
    route_id:        Option<&'static route::Id>,
    target_waypoint: Option<&'static nav::TargetWaypoint>,
    entity:          Entity,
}

#[derive(SystemParam)]
pub struct WriteRouteParams<'w, 's> {
    waypoint_query:         Query<'w, 's, &'static Waypoint>,
    waypoint_presets_query: Query<'w, 's, &'static route::WaypointPresetList>,
    preset_query:           Query<'w, 's, &'static route::Preset>,
    runway_query:           Query<'w, 's, (&'static Runway, &'static Waypoint)>,
    aerodrome_query:        Query<'w, 's, &'static Aerodrome>,
    commands:               Commands<'w, 's>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteRouteParams<'w, 's>;

    fn title() -> &'static str { "Route" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        if let Some(target) = this.target_waypoint {
            if let Ok(presets) = params.waypoint_presets_query.get(target.waypoint_entity) {
                write_route_options(
                    ui,
                    &params.preset_query,
                    &mut params.commands,
                    this.entity,
                    presets,
                    this.route_id.and_then(|id| id.0.as_deref()),
                );
            }
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
) {
    #[derive(Clone, Copy, PartialEq)]
    enum Selection {
        None,
        Available(usize),
        Retain,
    }

    let presets: Vec<_> = presets
        .iter()
        .filter_map(|entity| {
            Some(try_log!(
                preset_query.get(entity),
                expect "WaypointPresetList member {entity:?} should be a preset entity"
                or return None
            ))
        })
        .collect();

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

    egui::ComboBox::from_label("Standard route")
        .selected_text(current_route_id.unwrap_or("None"))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selection, Selection::None, "None");
            if current_selection == Selection::Retain {
                ui.selectable_value(&mut selection, Selection::Retain, current_route_id.unwrap());
            }
            for (i, preset) in presets.iter().enumerate() {
                ui.selectable_value(&mut selection, Selection::Available(i), &preset.title);
            }
        });
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
            let waypoint = try_log_return!(params.waypoint_query.get(node.waypoint), expect "route must reference valid waypoint {:?}", node.waypoint);
            match node.proximity {
                route::WaypointProximity::FlyBy => ui.label(format!("Fly by {}", &waypoint.name)),
                route::WaypointProximity::FlyOver => {
                    ui.label(format!("Fly over {}", &waypoint.name))
                }
            };

            if let Some(altitude) = node.altitude {
                struct Indent;
                ui.indent(TypeId::of::<Indent>(), |ui| {
                    ui.label(format!("Pass at altitude {:.0} ft", altitude.amsl().into_feet()));
                });
            }
        }
        route::Node::SetAirSpeed(node) => {
            ui.label(format!("Set speed to {:.0} kt", node.speed.into_knots()));
            if let Some(error) = node.error {
                struct Indent;
                ui.indent(TypeId::of::<Indent>(), |ui| {
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
                struct Indent;
                ui.indent(TypeId::of::<Indent>(), |ui| {
                    ui.label(format!("Maintain until \u{b1}{:.0} ft", error.into_feet()));
                });
            }
        }
        route::Node::AlignRunway(node) => {
            let (runway, waypoint) = try_log_return!(params.runway_query.get(node.runway), expect "route must reference valid runway {:?}", node.runway);
            let aerodrome = try_log_return!(params.aerodrome_query.get(runway.aerodrome), expect "runway must reference valid aerodrome {:?}", runway.aerodrome);
            ui.label(format!("Align towards runway {} of {}", &waypoint.name, &aerodrome.name));
        }
    }
}

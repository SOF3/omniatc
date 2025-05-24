use std::any::TypeId;

use bevy::app::{App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, ParamSet, Query, Res, ResMut, SystemParam};
use bevy_egui::{egui, EguiContextPass, EguiContexts};
use omniatc::level::aerodrome::Aerodrome;
use omniatc::level::route::{self, Route};
use omniatc::level::runway::Runway;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{nav, object, plane, wake, wind};
use omniatc::math::Sign;
use omniatc::try_log_return;
use omniatc::units::{Angle, TurnDirection};

use super::Writer;

#[derive(QueryData)]
pub struct ObjectQuery {
    route:  Option<&'static Route>,
    entity: Entity,
}

#[derive(SystemParam)]
pub struct WriteRouteParams<'w, 's> {
    waypoint:  Query<'w, 's, &'static Waypoint>,
    runway:    Query<'w, 's, (&'static Runway, &'static Waypoint)>,
    aerodrome: Query<'w, 's, &'static Aerodrome>,
    commands:  Commands<'w, 's>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteRouteParams<'w, 's>;

    fn title() -> &'static str { "Route" }

    fn should_show(this: &Self::Item<'_>) -> bool {
        this.route.is_some_and(|r| r.current().is_some())
    }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        let Some(route) = this.route else { return };

        for node in route.iter() {
            write_route_node(ui, node, this.entity, params);
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
            let waypoint = try_log_return!(params.waypoint.get(node.waypoint), expect "route must reference valid waypoint {:?}", node.waypoint);
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
            let (runway, waypoint) = try_log_return!(params.runway.get(node.runway), expect "route must reference valid runway {:?}", node.runway);
            let aerodrome = try_log_return!(params.aerodrome.get(runway.aerodrome), expect "runway must reference valid aerodrome {:?}", runway.aerodrome);
            ui.label(format!("Align towards runway {} of {}", &waypoint.name, &aerodrome.name));
        }
    }
}

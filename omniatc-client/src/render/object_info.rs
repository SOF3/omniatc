use bevy::app::{App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy_egui::{egui, EguiContextPass, EguiContexts};
use omniatc_core::level::aerodrome::Aerodrome;
use omniatc_core::level::waypoint::Waypoint;
use omniatc_core::level::{object, plane};
use omniatc_core::try_log_return;

use crate::{EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentHoveredObject>();
        app.init_resource::<CurrentObject>();
        app.add_systems(EguiContextPass, setup_layout_system.in_set(EguiSystemSets::ObjectInfo));
    }
}

#[derive(Default, Resource)]
pub struct CurrentHoveredObject(pub Option<Entity>);

#[derive(Default, Resource)]
pub struct CurrentObject(pub Option<Entity>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct CurrentObjectSelectorSystemSet;

fn setup_layout_system(
    mut contexts: EguiContexts,
    current_object: Res<CurrentObject>,
    object_query: Query<ObjectQueryData>,
    mut margins: ResMut<EguiUsedMargins>,
    write_params: Params,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };

    let width = egui::SidePanel::right("object_info")
        .resizable(true)
        .show(ctx, |ui| {
            let Some(object_entity) = current_object.0 else {
                ui.label("Click on an aircraft to view details");
                return;
            };

            let object = try_log_return!(
                object_query.get(object_entity),
                expect "CurrentObject points to non-object"
            );

            ui.heading(&object.display.name);
            egui::ScrollArea::vertical().show(ui, |ui| object.write(ui, &write_params));
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click());
        })
        .response
        .rect
        .width();
    margins.right += width;
}

#[derive(SystemParam)]
struct Params<'w, 's> {
    aerodrome_query: Query<'w, 's, &'static Aerodrome>,
    waypoint_query:  Query<'w, 's, &'static Waypoint>,
}

#[derive(QueryData)]
struct ObjectQueryData {
    dest:          &'static object::Destination,
    display:       &'static object::Display,
    object:        &'static object::Object,
    airborne:      Option<&'static object::Airborne>,
    plane_control: Option<&'static plane::Control>,
}

impl ObjectQueryDataItem<'_> {
    fn write(&self, ui: &mut egui::Ui, write_params: &Params) {
        egui::CollapsingHeader::new("Destination").default_open(true).show(ui, |ui| {
            ui.label(match *self.dest {
                object::Destination::Landing { aerodrome } => {
                    let data = try_log_return!(
                        write_params.aerodrome_query.get(aerodrome),
                        expect "Unknown aerodrome {aerodrome:?}"
                    );
                    format!("Arrival at {}", &data.name)
                }
                object::Destination::VacateAnyRunway => {
                    String::from("Land at any runway and vacate")
                }
                object::Destination::ReachWaypoint { min_altitude, waypoint_proximity } => {
                    let mut waypoint_name = None;
                    if let Some((waypoint_entity, _)) = waypoint_proximity {
                        if let Ok(data) = write_params.waypoint_query.get(waypoint_entity) {
                            waypoint_name = Some(&data.name);
                        }
                    }

                    match (min_altitude, waypoint_name) {
                        (Some(altitude), Some(waypoint)) => {
                            format!(
                                "Reach {waypoint:?} and climb past {:.0}ft",
                                altitude.amsl().into_feet()
                            )
                        }
                        (Some(altitude), None) => {
                            format!("Climb past {:.0}ft", altitude.amsl().into_feet())
                        }
                        (None, Some(waypoint)) => format!("Reach {waypoint:?}"),
                        (None, None) => String::from("None"),
                    }
                }
            });
        });

        egui::CollapsingHeader::new("Heading").default_open(true).show(ui, |ui| {
            if let Some(control) = &self.plane_control {
                ui.label(format!("Current: {:.0}\u{b0}", control.heading.degrees()));
            }
        });

        egui::CollapsingHeader::new("Altitude").default_open(true).show(ui, |ui| {
            ui.label(format!(
                "Current: {:.0} ft",
                self.object.position.altitude().amsl().into_feet()
            ));
            if let Some(airborne) = &self.airborne {
                ui.label(format!(
                    "Vert rate: {:.0} fpm",
                    airborne.airspeed.vertical().magnitude_exact().into_fpm()
                ));
            }
        });

        egui::CollapsingHeader::new("Speed").default_open(true).show(ui, |ui| {
            ui.label(format!(
                "Current ground: {:.0} kt",
                self.object.ground_speed.magnitude_exact().into_knots()
            ));
            if let Some(airborne) = &self.airborne {
                ui.label(format!(
                    "Current indicated air: {:.0} kt",
                    airborne.airspeed.horizontal().magnitude_exact().into_knots()
                ));
            }
        });
    }
}

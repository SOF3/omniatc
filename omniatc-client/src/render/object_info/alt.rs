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
    object:       &'static object::Object,
    airborne:     Option<&'static object::Airborne>,
    target_alt:   Option<&'static nav::TargetAltitude>,
    target_glide: Option<(&'static nav::TargetGlide, &'static nav::TargetGlideStatus)>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = Query<'w, 's, &'static Waypoint>;

    fn title() -> &'static str { "Altitude" }

    fn should_show(this: &Self::Item<'_>) -> bool { this.airborne.is_some() }

    fn show(
        this: &Self::Item<'_>,
        ui: &mut egui::Ui,
        waypoint_query: &mut Self::SystemParams<'_, '_>,
    ) {
        ui.label(format!("Current: {:.0} ft", this.object.position.altitude().amsl().into_feet()));
        if let Some(airborne) = this.airborne {
            ui.label(format!("Vert rate: {:+.0} fpm", airborne.airspeed.vertical().into_fpm()));
        }

        if let Some(target_alt) = this.target_alt {
            let expedite = if target_alt.expedite { " (expedite)" } else { "" };
            ui.label(format!("Target: {:.0} ft{expedite}", target_alt.altitude.amsl().into_feet()));
        }

        if let Some((glide, glide_status)) = this.target_glide {
            let waypoint = try_log_return!(
                waypoint_query.get(glide.target_waypoint),
                expect "TargetGlide has invalid waypoint {:?}", glide.target_waypoint,
            );
            let target_altitude = waypoint.position.altitude().amsl().into_feet();

            if glide.glide_angle.is_zero() {
                ui.label(format!("Target: maintain {target_altitude} ft until {}", &waypoint.name));
            } else if glide.glide_angle.is_positive() {
                ui.label(format!(
                    "Target: {}\u{b0} climb to {}",
                    glide.glide_angle.into_degrees(),
                    &waypoint.name
                ));
            } else {
                ui.label(format!(
                    "Target: {}\u{b0} descent to {}",
                    glide.glide_angle.into_degrees().abs(),
                    &waypoint.name
                ));
            }

            {
                struct Indent;

                ui.indent(TypeId::of::<Indent>(), |ui| {
                    ui.label(format!(
                        "Target pitch: {:.1}\u{b0}",
                        glide_status.current_pitch.into_degrees()
                    ));
                    ui.label(format!(
                        "Vertical deviation: {:+.0} ft",
                        glide_status.altitude_deviation.into_feet()
                    ));
                    ui.label(format!(
                        "Horizontal distance to glidepath: {:+.1} nm",
                        glide_status.glidepath_distance.into_nm()
                    ));
                });
            }
        }
    }
}

use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy_egui::egui;
use math::{Position, TROPOPAUSE_ALTITUDE};
use omniatc::QueryTryLog;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{comm, nav, object};

use super::Writer;
use crate::input;
use crate::util::new_type_id;

#[derive(QueryData)]
pub struct ObjectQuery {
    entity:       Entity,
    object:       &'static object::Object,
    airborne:     Option<&'static object::Airborne>,
    target_alt:   Option<&'static nav::TargetAltitude>,
    target_glide: Option<(&'static nav::TargetGlide, &'static nav::TargetGlideStatus)>,
}

#[derive(SystemParam)]
pub struct WriteParams<'w, 's> {
    waypoint_query: Query<'w, 's, &'static Waypoint>,
    instr_writer:   MessageWriter<'w, comm::InstructionMessage>,
    hotkeys:        Res<'w, input::Hotkeys>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteParams<'w, 's>;

    fn title() -> &'static str { "Altitude" }

    fn should_show(this: &Self::Item<'_, '_>) -> bool { this.airborne.is_some() }

    fn show(this: &Self::Item<'_, '_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        ui.label(format!("Current: {:.0} ft", this.object.position.altitude().amsl().into_feet()));
        if let Some(airborne) = this.airborne {
            ui.label(format!("Vert rate: {:+.0} fpm", airborne.airspeed.vertical().into_fpm()));
        }

        if let Some(target_alt) = this.target_alt {
            let expedite = if target_alt.expedite { " (expedite)" } else { "" };
            ui.label(format!("Target: {:.0} ft{expedite}", target_alt.altitude.amsl().into_feet()));
        }

        ui.horizontal(|ui| {
            let initial_alt = this
                .target_alt
                .map_or_else(|| this.object.position.altitude(), |t| t.altitude)
                .amsl()
                .into_feet();
            let mut slider_alt = initial_alt;
            let slider_resp = ui.add(egui::Slider::new(
                &mut slider_alt,
                0.0..=TROPOPAUSE_ALTITUDE.amsl().into_feet(),
            ));
            if params.hotkeys.set_altitude {
                slider_resp.request_focus();
            }
            if params.hotkeys.inc_altitude {
                slider_alt = (slider_alt / 1000.).floor() * 1000. + 1000.;
            }
            if params.hotkeys.dec_altitude {
                slider_alt = (slider_alt / 1000.).ceil() * 1000. - 1000.;
            }

            let expedite = this.target_alt.is_some_and(|t| t.expedite);
            let mut checkbox_expedite = expedite;
            ui.add(
                egui::Checkbox::new(&mut checkbox_expedite, "Exp")
                    .indeterminate(this.target_alt.is_none()),
            );
            if params.hotkeys.toggle_expedite {
                checkbox_expedite = !checkbox_expedite;
            }

            #[expect(clippy::float_cmp)] // this is normally equal if user did not interact
            if slider_alt != initial_alt
                || (this.target_alt.is_some() && expedite != checkbox_expedite)
            {
                params.instr_writer.write(comm::InstructionMessage {
                    object: this.entity,
                    body:   comm::SetAltitude {
                        target: nav::TargetAltitude {
                            altitude: Position::from_amsl_feet(slider_alt),
                            expedite: checkbox_expedite,
                        },
                    }
                    .into(),
                });
            }
        });

        if let Some((glide, glide_status)) = this.target_glide {
            display_glide(ui, params, glide, glide_status);
        }
    }
}

fn display_glide(
    ui: &mut egui::Ui,
    params: &mut WriteParams,
    glide: &nav::TargetGlide,
    glide_status: &nav::TargetGlideStatus,
) {
    let Some(waypoint) = params.waypoint_query.log_get(glide.target_waypoint) else { return };
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

    ui.indent(new_type_id!(), |ui| {
        ui.label(format!("Target pitch: {:.1}\u{b0}", glide_status.current_pitch.into_degrees()));
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

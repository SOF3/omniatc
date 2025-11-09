use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, Res, SystemParam};
use bevy_egui::egui;
use math::Speed;
use omniatc::level::instr::CommandsExt;
use omniatc::level::{instr, nav, object};

use super::Writer;
use crate::input;

#[derive(QueryData)]
pub struct ObjectQuery {
    entity:   Entity,
    object:   &'static object::Object,
    airborne: Option<&'static object::Airborne>,
    nav_vel:  Option<&'static nav::VelocityTarget>,
    ground:   Option<&'static object::OnGround>,
}

#[derive(SystemParam)]
pub struct WriteParams<'w, 's> {
    commands: Commands<'w, 's>,
    hotkeys:  Res<'w, input::Hotkeys>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteParams<'w, 's>;

    fn title() -> &'static str { "Speed" }

    fn should_show(_this: &Self::Item<'_, '_>) -> bool { true }

    fn show(this: &Self::Item<'_, '_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        ui.label(format!(
            "Current ground: {:.0} kt",
            this.object.ground_speed.magnitude_exact().into_knots()
        ));
        if let Some(airborne) = this.airborne {
            ui.label(format!(
                "Current true airspeed: {:.0} kt",
                airborne.true_airspeed.horizontal().magnitude_exact().into_knots()
            ));
            ui.label(format!(
                "Current indicated airspeed: {:.0} kt",
                airborne.airspeed.horizontal().magnitude_exact().into_knots()
            ));

            if let Some(nav_vel) = this.nav_vel {
                let target_knots = nav_vel.horiz_speed.into_knots();
                ui.label(format!("Target IAS: {target_knots:.0} kt"));

                let mut slider_knots = target_knots;
                let slider_resp = ui
                    .add(egui::Slider::new(&mut slider_knots, 0. ..=300.).step_by(1.).suffix("kt"));
                if params.hotkeys.set_speed {
                    slider_resp.request_focus();
                }
                if params.hotkeys.inc_speed {
                    slider_knots = (slider_knots / 10.).floor() * 10. + 10.;
                }
                if params.hotkeys.dec_speed {
                    slider_knots = (slider_knots / 10.).ceil() * 10. - 10.;
                }

                if (target_knots - slider_knots).abs() > 1.0 {
                    params.commands.send_instruction(
                        this.entity,
                        instr::SetSpeed { target: Speed::from_knots(slider_knots) },
                    );
                }
            }
        } else if let Some(ground) = this.ground {
            match ground.target_speed {
                object::OnGroundTargetSpeed::Exact(speed) => {
                    ui.label(format!(
                        "Target speed: {:.0} kt",
                        speed.magnitude_exact().into_knots()
                    ));
                }
                object::OnGroundTargetSpeed::TakeoffRoll => {
                    ui.label("Rolling to takeoff");
                }
            }
        }
    }
}

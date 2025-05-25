use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, SystemParam};
use bevy_egui::egui;
use omniatc::level::{comm, nav, object};
use omniatc::units::Speed;

use super::Writer;

#[derive(QueryData)]
pub struct ObjectQuery {
    entity:   Entity,
    object:   &'static object::Object,
    airborne: Option<&'static object::Airborne>,
    nav_vel:  Option<&'static nav::VelocityTarget>,
}

#[derive(SystemParam)]
pub struct WriteParams<'w, 's> {
    commands: Commands<'w, 's>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteParams<'w, 's>;

    fn title() -> &'static str { "Speed" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
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
        }
        if let Some(nav_vel) = this.nav_vel {
            let target_knots = nav_vel.horiz_speed.into_knots();
            ui.label(format!("Target IAS: {target_knots:.0} kt"));

            let mut slider_knots = target_knots;
            ui.add(egui::Slider::new(&mut slider_knots, 0. ..=300.).suffix('\u{b0}'));

            #[expect(clippy::float_cmp)] // this is normally equal if user did not interact
            if target_knots != slider_knots {
                params.commands.send_event(comm::InstructionEvent {
                    object: this.entity,
                    body:   comm::SetSpeed { target: Speed::from_knots(slider_knots) }.into(),
                });
            }
        }
    }
}

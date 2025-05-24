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
    object:   &'static object::Object,
    airborne: Option<&'static object::Airborne>,
    nav_vel:  Option<&'static nav::VelocityTarget>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = ();

    fn title() -> &'static str { "Speed" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, (): &mut Self::SystemParams<'_, '_>) {
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
            ui.label(format!("Target IAS: {:.0} kt", nav_vel.horiz_speed.into_knots()));
        }
    }
}

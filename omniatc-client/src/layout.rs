use bevy::{app::{App, Plugin}, ecs::schedule::IntoScheduleConfigs};
use bevy_egui::{egui, EguiContextPass, EguiContexts};

use crate::EguiSystemSets;

// TODO level management GUI
pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiContextPass, setup_panels_system.in_set(EguiSystemSets::Setup));
    }
}

fn setup_panels_system(
    mut contexts: EguiContexts,
) {
    let ctx = contexts.ctx_mut();
    egui::CentralPanel::default().show(&ctx, |ui| {

    });
}

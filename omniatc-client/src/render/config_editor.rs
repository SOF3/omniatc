use bevy::app::{App, Plugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bevy_mod_config::manager;

use crate::EguiSystemSets;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Opened>();
        app.add_systems(EguiPrimaryContextPass, setup_window_system.in_set(EguiSystemSets::Config));
    }
}

#[derive(Default, Resource)]
pub struct Opened(pub bool);

fn setup_window_system(
    mut contexts: EguiContexts,
    mut opened: ResMut<Opened>,
    mut manager: manager::egui::Display,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let default_size = ctx.screen_rect().size() / 2.;
    egui::Window::new("Settings")
        .default_size(default_size)
        .default_open(false)
        .open(&mut opened.0)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200),
            ..Default::default()
        })
        .show(ctx, |ui| manager.show(ui));
}

#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)] // too many false positives from system params.
#![allow(dead_code)] // TODO remove this in the future

use std::time::Duration;

use bevy::app::{self, App};
use bevy::ecs::schedule::{self, ScheduleBuildSettings};
use bevy::prelude::IntoSystemSetConfigs;
use bevy::winit::WinitSettings;

mod level;
mod math;
mod pid;
mod ui;

fn main() {
    App::new()
        .add_plugins((
            bevy::DefaultPlugins,
            #[cfg(feature = "inspect")]
            bevy_inspector_egui::quick::WorldInspectorPlugin::new(),
            level::Plug,
            ui::Plug,
        ))
        .configure_sets(app::Update, level::SystemSets::All.after(ui::SystemSets::Input))
        .configure_sets(app::Update, level::SystemSets::All.before(ui::SystemSets::RenderAll))
        .edit_schedule(app::Update, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: schedule::LogLevel::Warn,
                ..Default::default()
            });
        })
        .insert_resource(WinitSettings {
            focused_mode:   bevy::winit::UpdateMode::reactive_low_power(Duration::from_millis(50)),
            unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(Duration::from_millis(500)),
        })
        .run();
}

#![warn(clippy::pedantic)]
#![cfg_attr(feature = "precommit-checks", deny(warnings, clippy::pedantic))]
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)] // too many false positives from system params.
#![cfg_attr(not(feature = "precommit-checks"), allow(dead_code, unused_imports))]
#![cfg_attr(feature = "precommit-checks", deny(clippy::dbg_macro))]
#![cfg_attr(feature = "precommit-checks", allow(dead_code))] // TODO remove this in the future

use std::time::Duration;

use bevy::app::{self, App, PluginGroup};
use bevy::ecs::schedule::{self, ScheduleBuildSettings};
use bevy::prelude::IntoSystemSetConfigs;
use bevy::window::{Window, WindowPlugin};
use bevy::winit::WinitSettings;
use omniatc_core::level;

mod ui;

fn main() {
    App::new()
        .add_plugins((
            bevy::DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window { fit_canvas_to_parent: true, ..Default::default() }),
                ..Default::default()
            }),
            #[cfg(feature = "inspect")]
            bevy_inspector_egui::quick::WorldInspectorPlugin::new(),
            level::Plug,
            ui::Plug,
        ))
        .configure_sets(app::Update, level::AllSystemSets.after(ui::SystemSets::Input))
        .configure_sets(app::Update, level::AllSystemSets.before(ui::SystemSets::RenderAll))
        .edit_schedule(app::Update, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: schedule::LogLevel::Warn,
                ..Default::default()
            });
        })
        .insert_resource(WinitSettings {
            focused_mode:   bevy::winit::UpdateMode::reactive_low_power(Duration::from_millis(10)),
            unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(Duration::from_millis(500)),
        })
        .run();
}

#![warn(clippy::pedantic)]
#![cfg_attr(feature = "precommit-checks", deny(warnings, clippy::pedantic, clippy::dbg_macro))]
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)] // too many false positives from system params.
#![allow(clippy::collapsible_else_if)] // this is usually intentional
#![allow(clippy::missing_panics_doc)] // 5:21 PM conrad.lock().expect("luscious")[tty0] : Worst clippy lint
#![cfg_attr(not(feature = "precommit-checks"), allow(dead_code, unused_variables, unused_imports))]
#![cfg_attr(feature = "precommit-checks", allow(dead_code))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", warn(warnings, clippy::pedantic, clippy::dbg_macro))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", allow(unused_imports))] // TODO remove this in the future

use std::time::Duration;

use bevy::app::{self, App, PluginGroup};
use bevy::asset::AssetPlugin;
use bevy::diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{self, IntoScheduleConfigs, ScheduleBuildSettings, SystemSet};
use bevy::ecs::system::ResMut;
use bevy::render::RenderPlugin;
use bevy::render::settings::{RenderCreation, WgpuLimits, WgpuSettings};
use bevy::window::{Window, WindowPlugin};
use bevy::winit::WinitSettings;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use bevy_mod_config::manager;
use itertools::Itertools;
use omniatc::level;
use strum::IntoEnumIterator;

mod input;
mod render;
mod storage;
mod util;

type ConfigManager = (manager::Egui, manager::serde::json::Pretty);

#[derive(clap::Parser)]
#[clap(version, about)]
struct Args {
    /// Path to the assets directory.
    #[clap(long, default_value = "assets")]
    assets_dir: String,
}

fn main() {
    let clap = <Args as clap::Parser>::parse();

    let mut app = App::new();
    app.add_plugins((
        bevy::DefaultPlugins
            .set(AssetPlugin { file_path: clap.assets_dir, ..Default::default() })
            .set(WindowPlugin {
                primary_window: Some(Window { fit_canvas_to_parent: true, ..Default::default() }),
                ..Default::default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    limits: WgpuLimits { max_texture_dimension_2d: 8192, ..Default::default() },
                    ..Default::default()
                }),
                ..Default::default()
            }),
        EntityCountDiagnosticsPlugin::default(),
        FrameTimeDiagnosticsPlugin::default(),
        bevy_egui::EguiPlugin::default(),
        level::Plug::<ConfigManager>::default(),
        omniatc::load::Plug,
        omniatc::util::Plug,
        input::Plug,
        render::Plug,
        storage::plugin(),
        util::billboard::Plug,
        util::shapes::Plug,
    ));

    app.configure_sets(app::Update, level::AllSystemSets.in_set(UpdateSystemSets::Simulate));
    for (before, after) in UpdateSystemSets::iter().tuple_windows() {
        app.configure_sets(app::Update, before.before(after));
    }

    app.edit_schedule(app::Update, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: schedule::LogLevel::Warn,
            ..Default::default()
        });
    });

    for (before, after) in EguiSystemSets::iter().tuple_windows() {
        app.configure_sets(EguiPrimaryContextPass, before.before(after));
    }

    app.init_resource::<EguiUsedMargins>();
    app.add_systems(
        EguiPrimaryContextPass,
        EguiUsedMargins::reset_system.in_set(EguiSystemSets::Init),
    );

    app.edit_schedule(EguiPrimaryContextPass, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: schedule::LogLevel::Warn,
            ..Default::default()
        });
    });

    app.insert_resource(WinitSettings {
        focused_mode:   bevy::winit::UpdateMode::reactive_low_power(Duration::from_millis(10)),
        unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(Duration::from_millis(500)),
    });

    app.run();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
enum UpdateSystemSets {
    Input,
    Simulate,
    Render,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
enum EguiSystemSets {
    Init,
    LevelInfo,
    ObjectInfo,
    Messages,
    Config,
    TwoDim,
}

#[derive(Debug, Resource, Default)]
struct EguiUsedMargins {
    left:   f32,
    right:  f32,
    top:    f32,
    bottom: f32,

    /// Whether pointer is used by some egui component.
    pointer_acquired:  bool,
    /// Whether keyboard input is used by some egui component.
    keyboard_acquired: bool,
}

impl EguiUsedMargins {
    fn reset_system(mut margins: ResMut<Self>, mut contexts: EguiContexts) {
        *margins = Self::default();

        if let Ok(ctx) = contexts.ctx_mut() {
            margins.pointer_acquired = ctx.wants_pointer_input();
            margins.keyboard_acquired = ctx.wants_keyboard_input();
        }
    }
}

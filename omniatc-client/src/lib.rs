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
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bevy_mod_config::manager;
use itertools::Itertools;
use omniatc::level;
use strum::IntoEnumIterator;

pub mod input;
pub mod render;
mod storage;
pub mod util;

type ConfigManager = (manager::Egui, manager::serde::json::Pretty);

#[derive(clap::Parser)]
#[clap(version, about)]
pub struct Options {
    /// Path to the assets directory.
    #[clap(long, default_value = "assets")]
    pub assets_dir: String,
}

pub fn main_app(options: Options) -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::DefaultPlugins
            .set(AssetPlugin { file_path: options.assets_dir, ..Default::default() })
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
        #[cfg(feature = "debug")]
        bevy_inspector_egui::quick::WorldInspectorPlugin::new(),
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

    app
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
pub enum UpdateSystemSets {
    Input,
    Simulate,
    Render,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
pub enum EguiSystemSets {
    Init,
    LevelInfo,
    ObjectInfo,
    Messages,
    Tutorial,
    TwoDim,
}

#[derive(Debug, Resource, Default)]
pub struct EguiUsedMargins {
    pub left:   f32,
    pub right:  f32,
    pub top:    f32,
    pub bottom: f32,

    /// Whether pointer is used by some egui component.
    pub pointer_acquired:  bool,
    /// Whether keyboard input is used by some egui component.
    pub keyboard_acquired: bool,
}

impl EguiUsedMargins {
    fn reset_system(mut margins: ResMut<Self>, mut contexts: EguiContexts) {
        *margins = Self::default();

        if let Ok(ctx) = contexts.ctx_mut() {
            margins.pointer_acquired = ctx.wants_pointer_input();
            margins.keyboard_acquired = ctx.wants_keyboard_input();

            ctx.set_theme(egui::ThemePreference::Dark);
            egui_material_icons::initialize(ctx);
        }
    }
}

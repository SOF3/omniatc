use std::time::Duration;

use bevy::app::{self, App, PluginGroup};
use bevy::asset::AssetPlugin;
use bevy::diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{self, IntoScheduleConfigs, ScheduleBuildSettings, SystemSet};
use bevy::ecs::system::ResMut;
use bevy::render::RenderPlugin;
use bevy::render::settings::{RenderCreation, WgpuLimits, WgpuSettings};
use bevy::time::TimeUpdateStrategy;
use bevy::window::{Window, WindowPlugin};
use bevy::winit::{WinitPlugin, WinitSettings};
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

    /// Disable the default winit and time plugins,
    /// for headless testing with synthetic window handles.
    #[clap(skip = false)]
    pub headless_test: bool,

    /// Open the specified level on startup, if any.
    pub open_level_id:    Option<String>,
    #[clap(long, default_value = storage::scenario_loader::DEFAULT_SCENARIO)]
    pub default_scenario: String,
}

pub fn main_app(options: Options) -> App {
    let mut app = App::new();

    app.add_plugins({
        let mut default_plugins = bevy::DefaultPlugins
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
            });
        if options.headless_test {
            // When running headless integration tests we override winit setup with a synthetic window,
            // so we disable the default plugin here to avoid event loop conflicts.
            default_plugins = default_plugins.disable::<WinitPlugin>();
        }
        default_plugins
    });

    if options.headless_test {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)));
    }

    app.add_plugins((
        EntityCountDiagnosticsPlugin::default(),
        FrameTimeDiagnosticsPlugin::default(),
        bevy_egui::EguiPlugin::default(),
        #[cfg(feature = "debug")]
        bevy_inspector_egui::quick::WorldInspectorPlugin::new(),
    ));

    app.add_plugins((
        level::Plug::<ConfigManager>::default(),
        omniatc::load::Plug,
        omniatc::util::Plug,
        input::Plug,
        render::Plug,
        storage::plugin(storage::StartupLevelOptions {
            open_level_id:    options.open_level_id,
            default_scenario: options.default_scenario,
        }),
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

    app.init_resource::<EguiState>();
    app.add_systems(
        EguiPrimaryContextPass,
        EguiState::reset_frame_system.in_set(EguiSystemSets::Init),
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
    MenuBar,
    ManageTabs,
    Dock,
    TutorialPopup,
}

#[derive(Debug, Resource, Default)]
pub struct EguiState {
    /// Whether pointer is used by some egui component.
    pub pointer_acquired:  bool,
    /// Whether keyboard input is used by some egui component.
    pub keyboard_acquired: bool,
}

impl EguiState {
    fn reset_frame_system(mut margins: ResMut<Self>, mut contexts: EguiContexts) {
        *margins = Self::default();

        if let Ok(ctx) = contexts.ctx_mut() {
            margins.pointer_acquired = ctx.wants_pointer_input();
            margins.keyboard_acquired = ctx.wants_keyboard_input();

            ctx.set_theme(egui::ThemePreference::Dark);
            egui_material_icons::initialize(ctx);
        }
    }
}

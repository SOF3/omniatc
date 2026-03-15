use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::asset::Assets;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, Camera2d, ImageRenderTarget, RenderTarget};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{self, IntoScheduleConfigs, Schedulable, ScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, Local, ParamSet, Query, ResMut, SystemParam};
use bevy::image::Image;
use bevy::render::render_resource::{
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_egui::egui::WidgetText;
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPrimaryContextPass, EguiTextureHandle, EguiUserTextures,
    PrimaryEguiContext, egui,
};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState};
use itertools::Itertools;
use strum::IntoEnumIterator;

use crate::EguiSystemSets;
use crate::util::new_type_id;

mod config_editor;
mod dock;
mod level_info;
mod messages;
mod object_info;
pub mod threedim;
mod tutorial_popup;
pub mod twodim;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            dock::Plug,
            messages::Plug,
            config_editor::Plug,
            level_info::Plug,
            object_info::Plug,
            tutorial_popup::Plug,
            twodim::Plug,
        ));

        for set in SystemSets::iter() {
            app.configure_sets(app::Update, set.in_set(crate::UpdateSystemSets::Render));
        }
        for (before, after) in SystemSets::iter().tuple_windows() {
            app.configure_sets(app::Update, before.before(after));
        }

        app.configure_sets(app::Update, SystemSets::Spawn.ambiguous_with(SystemSets::Spawn));
        app.configure_sets(app::Update, SystemSets::Update.ambiguous_with(SystemSets::Update));

        app.add_systems(
            EguiPrimaryContextPass,
            render_menu_bar_system.in_set(EguiSystemSets::MenuBar),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
pub enum SystemSets {
    /// Update existing entities for config changes.
    Reload,
    /// Spawn new entities.
    Spawn,
    /// Update existing entities regularly.
    Update,
}

#[derive(Component)]
#[require(MenuButtonClicked)]
pub struct MenuButton {
    pub icon:     &'static str,
    pub title:    String,
    pub group:    MenuButtonGroup,
    pub priority: i32,
}

#[derive(Default, Component)]
pub struct MenuButtonClicked {
    pub clicked: bool,
}

impl MenuButtonClicked {
    pub fn consume(&mut self) -> bool { mem::take(&mut self.clicked) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MenuButtonGroup {
    Game,
    Level,
    Object,
}

fn render_menu_bar_system(
    mut contexts: EguiContexts,
    query: Query<(&MenuButton, &mut MenuButtonClicked)>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut buttons: Vec<_> = query.into_iter().collect();
    buttons.sort_by_key(|(button, _)| (button.group, button.priority));
    if let Some((a, b)) = buttons
        .array_windows()
        .map(|[(a, _), (b, _)]| (a, b))
        .find(|(a, b)| a.group == b.group && a.priority == b.priority)
    {
        bevy::log::warn!("Ambiguous ordering between menu buttons {} and {}", a.title, b.title);
    }

    egui::TopBottomPanel::top(new_type_id!()).show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            for (group_index, (_, group)) in
                buttons.into_iter().chunk_by(|(button, _)| button.group).into_iter().enumerate()
            {
                if group_index > 0 {
                    ui.separator();
                }

                for (button, mut clicked) in group {
                    if ui
                        .button(egui::RichText::new(button.icon))
                        .on_hover_text(&button.title)
                        .clicked()
                    {
                        clicked.clicked = true;
                    }
                }
            }
        });
    });
}

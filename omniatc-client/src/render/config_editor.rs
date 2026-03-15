use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{ResMut, Single};
use bevy_egui::{EguiPrimaryContextPass, egui};
use egui_material_icons::icons;

use crate::render::dock::TabPlacement;
use crate::render::{MenuButton, MenuButtonClicked, dock};
use crate::{EguiSystemSets, render};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.world_mut().spawn((
            MenuButton {
                icon:     icons::ICON_SETTINGS,
                title:    "Settings".into(),
                group:    render::MenuButtonGroup::Game,
                priority: 100,
            },
            ConfigMenuButtonMarker,
        ));

        app.add_systems(
            EguiPrimaryContextPass,
            open_editor_system.in_set(EguiSystemSets::ManageTabs),
        );
    }
}

fn open_editor_system(
    mut dock_state: ResMut<dock::State>,
    mut menu_button_clicked: Single<&mut MenuButtonClicked, With<ConfigMenuButtonMarker>>,
) {
    if menu_button_clicked.consume()
        && let Some(state) = &mut dock_state.state
    {
        dock::focus_or_create_tab(
            state,
            || dock::Tab::ConfigEditor(TabType),
            dock::ReplaceTab(|tab| matches!(tab, dock::Tab::ConfigEditor(_)))
                .or_always(dock::NewSurface),
        );
    }
}

#[derive(Component)]
struct ConfigMenuButtonMarker;

pub struct TabType;

impl dock::TabType for TabType {
    type TitleSystemParam<'w, 's> = ();
    fn title(&self, (): ()) -> String { "Settings".into() }

    type UiSystemParam<'w, 's> = bevy_mod_config::manager::egui::Display<'w, 's>;
    fn ui(&mut self, mut param: Self::UiSystemParam<'_, '_>, ui: &mut egui::Ui, _order: usize) {
        param.show(ui);
    }

    type OnCloseSystemParam<'w, 's> = ();

    type PrepareRenderSystemParam<'w, 's> = ();
}

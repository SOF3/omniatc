use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, Single, SystemParam};
use bevy::time::Time;
use bevy_egui::{EguiPrimaryContextPass, egui};
use bevy_mod_config::ReadConfig;
use egui_dock::DockState;
use egui_material_icons::icons;
use omniatc::QueryTryLog;
use omniatc::level::instr::CommandsExt;
use omniatc::level::{instr, object, quest};

use crate::render::dock::{self, State, Tab, TabPlacement};
use crate::render::tutorial_popup;
use crate::{EguiSystemSets, UpdateSystemSets, input};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentHoveredObject>();
        app.init_resource::<CurrentObject>();
        app.init_resource::<DraftInstructions>();

        app.add_systems(
            app::Update,
            highlight_selected_system
                .after(UpdateSystemSets::Input)
                .after(CurrentObjectSelectorSystemSet)
                .in_set(super::twodim::object::SetColorThemeSystemSet::UserInteract),
        );

        app.add_systems(
            app::Update,
            cleanup_despawned_selected_object_system.before(CurrentObjectSelectorSystemSet),
        );
        app.add_systems(
            app::Update,
            send_selection_ui_event_system
                .in_set(quest::UiEventWriterSystemSet)
                .after(CurrentObjectSelectorSystemSet),
        );
        app.add_systems(
            EguiPrimaryContextPass,
            selection_to_tab_system.in_set(EguiSystemSets::ManageTabs),
        );
    }
}

#[derive(Default, Resource)]
struct DraftInstructions {
    airborne_vector: Option<instr::AirborneVector>,
}

fn cleanup_despawned_selected_object_system(
    mut despawn_events: MessageReader<object::DespawnMessage>,
    mut current_object: ResMut<CurrentObject>,
    mut current_hovered_object: ResMut<CurrentHoveredObject>,
) {
    for event in despawn_events.read() {
        if let Some(current) = current_object.0
            && current == event.0
        {
            current_object.0 = None;
        }
        if let Some(current) = current_hovered_object.0
            && current == event.0
        {
            current_hovered_object.0 = None;
        }
    }
}

#[derive(Default, Resource)]
pub struct CurrentHoveredObject(pub Option<Entity>);

/// Current object the user selected.
#[derive(Default, Resource)]
pub struct CurrentObject(pub Option<Entity>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct CurrentObjectSelectorSystemSet;

#[derive(SystemParam)]
struct SendParams<'w, 's> {
    draft:    ResMut<'w, DraftInstructions>,
    commands: Commands<'w, 's>,
    hotkeys:  Res<'w, input::Hotkeys>,
}

trait Writer: QueryData {
    type SystemParams<'w, 's>: SystemParam;

    fn title() -> &'static str;

    fn default_open() -> bool { true }

    fn should_show(this: &Self::Item<'_, '_>) -> bool;

    fn show(this: &Self::Item<'_, '_>, ui: &mut egui::Ui, param: &mut Self::SystemParams<'_, '_>);
}

macro_rules! writer_def {
    ($($index:ident: $writer:ty,)*) => {
        #[derive(QueryData)]
        struct WriteQueryData {
            $(
                $index: $writer,
            )*
        }

        #[derive(SystemParam)]
        pub struct WriteParams<'w, 's> {
            sets: ParamSet<'w, 's, ($(<$writer as Writer>::SystemParams<'w, 's>,)*)>,
        }

        fn show_writers(ui: &mut egui::Ui, qd: &WriteQueryDataItem, params: &mut WriteParams) {
            $(
                {
                    let qd = &qd.$index;

                    if <$writer as Writer>::should_show(qd) {
                        let mut params = params.sets.$index();
                        egui::CollapsingHeader::new(<$writer as Writer>::title())
                            .default_open(<$writer as Writer>::default_open())
                            .show(ui, |ui| {
                                <$writer as Writer>::show(qd, ui, &mut params);
                            });
                    }
                }
            )*
        }
    }
}

writer_def! {
    p0: dest::ObjectQuery,
    p1: dir::ObjectQuery,
    p2: alt::ObjectQuery,
    p3: speed::ObjectQuery,
    p4: env::ObjectQuery,
    p5: signal::ObjectQuery,
    p6: route::ObjectQuery,
}

mod alt;
mod dest;
mod dir;
mod env;
mod route;
mod signal;
mod speed;

fn highlight_selected_system(
    conf: ReadConfig<super::twodim::pick::Conf>,
    current_hovered_object: Res<CurrentHoveredObject>,
    current_object: Res<CurrentObject>,
    request_highlight: Option<
        Single<(), (With<tutorial_popup::Focused>, With<quest::highlight::ObjectSelect>)>,
    >,
    mut color_theme_query: Query<&mut super::twodim::object::ColorTheme>,
    time: Res<Time>,
) {
    let conf = conf.read();

    if let Some(entity) = current_hovered_object.0 {
        let Some(mut theme) = color_theme_query.log_get_mut(entity) else { return };
        theme.body = conf.hovered_color;
    }

    if let Some(entity) = current_object.0 {
        let Some(mut theme) = color_theme_query.log_get_mut(entity) else { return };
        theme.body = conf.selected_color;
    }

    if request_highlight.is_some() && time.elapsed().as_secs().is_multiple_of(2) {
        for mut theme in color_theme_query {
            theme.body = conf.tutorial_highlight_color;
            theme.ring = conf.tutorial_highlight_color;
        }
    }
}

fn send_selection_ui_event_system(
    mut ui_event_writer: MessageWriter<quest::UiEvent>,
    current_object: Res<CurrentObject>,
) {
    if current_object.0.is_some() {
        ui_event_writer.write(quest::UiEvent::ObjectSelected);
    }
}

pub enum TabType {
    /// Placeholder UI to keep the dock node when no objects are selected.
    Placeholder,
    /// Displays object information.
    Object {
        /// The object entity.
        object: Entity,
        /// Whether the tab should remain open even if the object is deselected.
        pinned: bool,
    },
}

#[derive(SystemParam)]
pub struct UiParams<'w, 's> {
    object_query: Query<'w, 's, (WriteQueryData, &'static object::Display)>,
    param_set:    ParamSet<'w, 's, (SendParams<'w, 's>, WriteParams<'w, 's>)>,
}

impl dock::TabType for TabType {
    type TitleSystemParam<'w, 's> = Query<'w, 's, &'static object::Display>;
    fn title(&self, param: Self::TitleSystemParam<'_, '_>) -> String {
        match *self {
            TabType::Placeholder => "Vehicle Info".into(),
            TabType::Object { object, pinned } => {
                let mut string = String::new();
                if pinned {
                    string.push_str(icons::ICON_PUSH_PIN);
                }
                string.push_str(if let Some(display) = param.log_get(object) {
                    display.name.as_str()
                } else {
                    "Unreachable vehicle"
                });
                string
            }
        }
    }

    type UiSystemParam<'w, 's> = UiParams<'w, 's>;
    fn ui(&mut self, mut params: Self::UiSystemParam<'_, '_>, ui: &mut egui::Ui, _order: usize) {
        let object_entity = match self {
            Self::Placeholder => {
                ui.label("Click on an aircraft to view details");
                params.param_set.p0().draft.airborne_vector = None;
                return;
            }
            Self::Object { object, .. } => *object,
        };

        let Some(object) = params.object_query.log_get(object_entity) else { return };

        ui.heading(&object.1.name);

        let mut send_params = params.param_set.p0();
        let send_clicked = ui
            .add_enabled(send_params.draft.airborne_vector.is_some(), egui::Button::new("Send"))
            .clicked();
        if (send_clicked || send_params.hotkeys.send)
            && let Some(instr) = send_params.draft.airborne_vector.take()
        {
            send_params.commands.send_instruction(object_entity, instr);
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            show_writers(ui, &object.0, &mut params.param_set.p1());
        });
    }

    type OnCloseSystemParam<'w, 's> = ();

    type PrepareRenderSystemParam<'w, 's> = ();
}

pub(super) fn create_splits(dock: &mut DockState<dock::Tab>) {
    dock.split(
        (egui_dock::SurfaceIndex::main(), egui_dock::NodeIndex::root()),
        egui_dock::Split::Right,
        0.7,
        egui_dock::Node::leaf(dock::Tab::ObjectInfo(TabType::Placeholder)),
    );
}

fn selection_to_tab_system(
    current_object: Res<CurrentObject>,
    mut dock_state: ResMut<State>,
    mut last_selection: Local<Option<Entity>>,
) {
    let Some(dock) = dock_state.state.as_mut() else { return };

    let last_selection = mem::replace(&mut *last_selection, current_object.0);
    if let Some(object) = current_object.0
        && last_selection != Some(object)
    {
        focus_or_create_tab(dock, object);
    }
}

fn focus_or_create_tab(dock: &mut DockState<dock::Tab>, object: Entity) {
    dock::focus_or_create_tab(
        dock,
        || Tab::ObjectInfo(TabType::Object { object, pinned: false }),
        dock::ReplaceTab(|tab| matches!(tab, Tab::ObjectInfo(TabType::Placeholder)))
            .or(dock::ReplaceTab(|tab| matches!(tab, Tab::ObjectInfo(TabType::Object { object: tab_obj, pinned: false }) if *tab_obj == object)))
            .or(dock::ReplaceTab(|tab| matches!(tab, Tab::ObjectInfo(TabType::Object { pinned: false, .. }))))
            .or(dock::AfterTab(|tab| matches!(tab, Tab::ObjectInfo(TabType::Object { pinned: true, .. }))))
            .or_always(dock::SplitRoot{split:egui_dock::Split::Right, ratio:0.7}),
    );
}

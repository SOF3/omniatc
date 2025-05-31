use bevy::app::{self, App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{ParamSet, Query, Res, ResMut, SystemParam};
use bevy_egui::{egui, EguiContextPass, EguiContexts};
use omniatc::level::object;
use omniatc::try_log_return;

use crate::{config, EguiSystemSets, EguiUsedMargins, UpdateSystemSets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentHoveredObject>();
        app.init_resource::<CurrentObject>();
        app.add_systems(EguiContextPass, setup_layout_system.in_set(EguiSystemSets::ObjectInfo));

        app.add_systems(
            app::Update,
            highlight_selected_system
                .after(UpdateSystemSets::Input)
                .in_set(super::twodim::object::SetColorThemeSystemSet::UserInteract),
        );
    }
}

#[derive(Default, Resource)]
pub struct CurrentHoveredObject(pub Option<Entity>);

/// Current object the user selected.
#[derive(Default, Resource)]
pub struct CurrentObject(pub Option<Entity>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct CurrentObjectSelectorSystemSet;

fn setup_layout_system(
    mut contexts: EguiContexts,
    current_object: Res<CurrentObject>,
    object_query: Query<(WriteQueryData, &object::Display)>,
    mut margins: ResMut<EguiUsedMargins>,
    mut write_params: WriteParams,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };

    let resp = egui::SidePanel::right("object_info")
        .resizable(true)
        .default_width(500.)
        .show(ctx, |ui| {
            let Some(object_entity) = current_object.0 else {
                ui.label("Click on an aircraft to view details");
                return;
            };

            let object = try_log_return!(
                object_query.get(object_entity),
                expect "CurrentObject points to non-object"
            );

            ui.heading(&object.1.name);
            egui::ScrollArea::vertical().show(ui, |ui| {
                show_writers(ui, &object.0, &mut write_params);
            });
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click());
        })
        .response;
    margins.right += resp.rect.width();
}

trait Writer: QueryData {
    type SystemParams<'w, 's>: SystemParam;

    fn title() -> &'static str;

    fn default_open() -> bool { true }

    fn should_show(this: &Self::Item<'_>) -> bool;

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, param: &mut Self::SystemParams<'_, '_>);
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
        struct WriteParams<'w, 's> {
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
    p5: route::ObjectQuery,
}

mod alt;
mod dest;
mod dir;
mod env;
mod route;
mod speed;

fn highlight_selected_system(
    conf: config::Read<super::twodim::pick::Conf>,
    current_hovered_object: Res<CurrentHoveredObject>,
    current_object: Res<CurrentObject>,
    mut color_theme_query: Query<&mut super::twodim::object::ColorTheme>,
) {
    if let Some(entity) = current_hovered_object.0 {
        let mut theme = try_log_return!(color_theme_query.get_mut(entity), expect "CurrentObject is Some and must reference valid object entity");
        theme.body = conf.hovered_color;
    }

    if let Some(entity) = current_object.0 {
        let mut theme = try_log_return!(color_theme_query.get_mut(entity), expect "CurrentObject is Some and must reference valid object entity");
        theme.body = conf.selected_color;
    }
}

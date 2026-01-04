use bevy::app::{App, Plugin};
use bevy::ecs::query::{Has, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Local, ParamSet, ResMut, Single, SystemParam};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use omniatc::level::quest::{self};
use strum::IntoEnumIterator;

use super::config_editor;
use crate::render::tutorial_popup;
use crate::util::new_type_id;
use crate::{EguiSystemSets, EguiUsedMargins};

mod camera;
mod diagnostics;
mod objects;
pub(super) mod quests;
mod score;
mod time;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            setup_layout_system.in_set(EguiSystemSets::LevelInfo),
        );
    }
}

trait WriteParams {
    fn title(&self) -> String;

    fn default_open() -> bool;

    fn write(&mut self, ui: &mut egui::Ui);

    fn display(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(self.title())
            .default_open(Self::default_open())
            .show(ui, |ui| self.write(ui));
    }
}

#[derive(SystemParam)]
struct TabListParams<'w, 's> {
    select_tab:           Local<'s, SelectTab>,
    config_editor_opened: ResMut<'w, config_editor::Opened>,
    focus_quest_query:
        Option<Single<'w, 's, Has<quest::highlight::LevelTab>, With<tutorial_popup::Focused>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::IntoStaticStr, strum::EnumIter)]
enum SelectTab {
    #[default]
    Level,
    Quests,
}

impl SelectTab {
    fn should_highlight(self, tab_params: &TabListParams) -> bool {
        match self {
            SelectTab::Level => tab_params.focus_quest_query.as_deref() == Some(&true),
            SelectTab::Quests => false,
        }
    }
}

fn setup_layout_system(
    mut contexts: EguiContexts,
    mut margins: ResMut<EguiUsedMargins>,
    mut write_params: ParamSet<(
        TabListParams,
        score::WriteScoreParams,
        time::WriteTimeParams,
        camera::WriteCameraParams,
        objects::WriteObjectParams,
        diagnostics::WriteDiagnosticsParams,
        quests::WriteQuestsParams,
    )>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let resp = egui::SidePanel::left(new_type_id!())
        .resizable(true)
        .show(ctx, |ui| {
            let mut tab_params = write_params.p0();

            ui.horizontal(|ui| {
                for option in SelectTab::iter() {
                    let button = egui::Button::selectable(
                        option == *tab_params.select_tab,
                        Into::<&'static str>::into(option),
                    );
                    let resp = if option.should_highlight(&tab_params) {
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(3.0, egui::Color32::RED))
                            .show(ui, |ui| ui.add(button))
                            .inner
                    } else {
                        ui.add(button)
                    };
                    if resp.clicked() {
                        *tab_params.select_tab = option;
                    }
                }

                if ui.selectable_label(false, "Settings").clicked() {
                    tab_params.config_editor_opened.0 = true;
                }
            });

            match *tab_params.select_tab {
                SelectTab::Level => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        write_params.p1().display(ui);
                        write_params.p2().display(ui);
                        write_params.p3().display(ui);
                        write_params.p4().display(ui);
                        write_params.p5().display(ui);
                    });
                }
                SelectTab::Quests => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        write_params.p6().display(ui);
                    });
                }
            }

            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click());
        })
        .response;
    margins.left += resp.rect.width();
}

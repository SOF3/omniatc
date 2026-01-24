use bevy::app::{App, Plugin};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Local, ParamSet, Res, ResMut, SystemParam};
use bevy::time::{Time, Virtual as TimeVirtual};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use strum::IntoEnumIterator;

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

    fn request_highlight(&self) -> Option<&RequestHighlightParams<'_>> { None }

    fn write(&mut self, ui: &mut egui::Ui);

    fn display(&mut self, ui: &mut egui::Ui) {
        let resp = egui::CollapsingHeader::new(self.title())
            .default_open(Self::default_open())
            .show(ui, |ui| self.write(ui));
        if let Some(hl_params) = self.request_highlight()
            && resp.body_response.is_none()
        {
            let elapsed = hl_params.time.elapsed();
            if elapsed.subsec_millis() < 500 {
                resp.header_response.show_tooltip_ui(|ui| {
                    egui::Frame::new().fill(egui::Color32::DARK_RED).show(ui, |ui| {
                        ui.label("Click to open");
                    });
                });
            }
        }
    }
}

#[derive(SystemParam)]
struct RequestHighlightParams<'w> {
    time: Res<'w, Time<TimeVirtual>>,
}

#[derive(SystemParam)]
struct TabListParams<'s> {
    select_tab: Local<'s, SelectTab>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::IntoStaticStr, strum::EnumIter)]
enum SelectTab {
    #[default]
    Level,
    Quests,
    ConfigEditor,
}

type LevelWriteParams<'w, 's> = (
    score::WriteScoreParams<'w>,
    time::WriteTimeParams<'w, 's>,
    camera::WriteCameraParams<'w, 's>,
    objects::WriteObjectParams<'w, 's>,
    diagnostics::WriteDiagnosticsParams<'w>,
);

macro_rules! each_write_params {
    ($set:expr, $mac:path, $state:tt) => {
        let mut set = $set;
        $mac!(set.p0(), $state);
        $mac!(set.p1(), $state);
        $mac!(set.p2(), $state);
        $mac!(set.p3(), $state);
        $mac!(set.p4(), $state);
    };
}

fn setup_layout_system(
    mut contexts: EguiContexts,
    mut margins: ResMut<EguiUsedMargins>,
    mut param_set: ParamSet<(
        TabListParams,
        ParamSet<LevelWriteParams>,
        quests::WriteQuestsParams,
        bevy_mod_config::manager::egui::Display,
    )>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let should_highlight_level = {
        macro_rules! each {
            ($p:expr, $state:tt) => {
                $state = $state || $p.request_highlight().is_some();
            };
        }

        let mut should_highlight = false;
        each_write_params!(param_set.p1(), each, should_highlight);
        should_highlight
    };

    let resp = egui::SidePanel::left(new_type_id!())
        .resizable(true)
        .show(ctx, |ui| {
            let mut tab_params = param_set.p0();

            ui.horizontal(|ui| {
                for option in SelectTab::iter() {
                    let button = egui::Button::selectable(
                        option == *tab_params.select_tab,
                        Into::<&'static str>::into(option),
                    );
                    let resp = if option == SelectTab::Level
                        && option != *tab_params.select_tab
                        && should_highlight_level
                    {
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
            });

            match *tab_params.select_tab {
                SelectTab::Level => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        macro_rules! each {
                            ($p:expr, $ui:tt) => {
                                $p.display($ui)
                            };
                        }
                        each_write_params!(param_set.p1(), each, ui);
                    });
                }
                SelectTab::Quests => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        param_set.p2().display(ui);
                    });
                }
                SelectTab::ConfigEditor => {
                    let mut manager = param_set.p3();
                    manager.show(ui);
                }
            }

            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click());
        })
        .response;
    margins.left += resp.rect.width();
}

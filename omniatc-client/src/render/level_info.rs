use bevy::app::{App, Plugin};
use bevy::ecs::schedule::{self, IntoScheduleConfigs, Schedulable, ScheduleConfigs};
use bevy::ecs::system::{ParamSet, Res, SystemParam};
use bevy::time::{Time, Virtual as TimeVirtual};
use bevy_egui::egui;
use egui_dock::DockState;
use omniatc::level::quest;

use crate::render::config_editor;
use crate::render::dock::{self, Tab};
use crate::render::object_info::CurrentObjectSelectorSystemSet;

mod camera;
mod diagnostics;
pub(super) mod objects;
pub(super) mod quests;
mod score;
mod time;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, _app: &mut App) {}
}

trait WriteParams {
    fn title(&self) -> String;

    fn default_open() -> bool;

    fn request_highlight(&self) -> Option<&RequestHighlightParams<'_>> { None }

    fn write(&mut self, ui: &mut egui::Ui);

    fn display(&mut self, ui: &mut egui::Ui) {
        let mut highlight = false;
        if let Some(hl_params) = self.request_highlight() {
            let elapsed = hl_params.time.elapsed();
            if elapsed.subsec_millis() < 500 {
                highlight = true;
            }
        }

        let mut title = egui::RichText::new(self.title());
        if highlight {
            title = title.color(egui::Color32::LIGHT_RED);
        }
        let resp = egui::CollapsingHeader::new(title)
            .default_open(Self::default_open())
            .show(ui, |ui| self.write(ui));
        if highlight && resp.body_response.is_none() {
            resp.header_response.show_tooltip_ui(|ui| {
                egui::Frame::new().fill(egui::Color32::DARK_RED).show(ui, |ui| {
                    ui.label("Click to open");
                });
            });
        }
    }
}

#[derive(SystemParam)]
struct RequestHighlightParams<'w> {
    time: Res<'w, Time<TimeVirtual>>,
}

#[derive(SystemParam)]
pub struct LevelWriteParams<'w, 's> {
    ps: ParamSet<
        'w,
        's,
        (
            score::WriteScoreParams<'w>,
            time::WriteTimeParams<'w, 's>,
            camera::WriteCameraParams<'w, 's>,
            diagnostics::WriteDiagnosticsParams<'w>,
            // NOTE: remember to update each_write_params upon adding an entry here
        ),
    >,
}

macro_rules! each_write_params {
    ($set:expr, $mac:path, $state:tt) => {
        let mut set = $set;
        $mac!(set.ps.p0(), $state);
        $mac!(set.ps.p1(), $state);
        $mac!(set.ps.p2(), $state);
        $mac!(set.ps.p3(), $state);
    };
}

pub struct ScalarTabType;

impl dock::TabType for ScalarTabType {
    type TitleSystemParam<'w, 's> = ();
    fn title(&self, (): ()) -> String { "World".into() }

    type UiSystemParam<'w, 's> = LevelWriteParams<'w, 's>;
    fn ui(&mut self, param_set: Self::UiSystemParam<'_, '_>, ui: &mut egui::Ui, _order: usize) {
        macro_rules! each {
            ($p:expr, $ui:tt) => {
                $p.display($ui)
            };
        }
        each_write_params!(param_set, each, ui);
    }

    fn schedule_configs<T>(configs: ScheduleConfigs<T>) -> ScheduleConfigs<T>
    where
        T: Schedulable<Metadata = schedule::GraphInfo, GroupMetadata = schedule::Chain>,
    {
        configs.in_set(quest::UiEventWriterSystemSet).in_set(CurrentObjectSelectorSystemSet)
    }

    type OnCloseSystemParam<'w, 's> = ();

    type PrepareRenderSystemParam<'w, 's> = ();
}

pub(super) fn create_splits(dock: &mut DockState<Tab>) {
    let [_, left] = dock.split(
        (egui_dock::SurfaceIndex::main(), egui_dock::NodeIndex::root()),
        egui_dock::Split::Left,
        0.3,
        egui_dock::Node::leaf_with(
            [
                Tab::LevelInfo(ScalarTabType),
                Tab::Quests(quests::TabType),
                Tab::ConfigEditor(config_editor::TabType),
            ]
            .into(),
        ),
    );
    dock.split(
        (egui_dock::SurfaceIndex::main(), left),
        egui_dock::Split::Below,
        0.5,
        egui_dock::Node::leaf(Tab::ObjectList(objects::TabType)),
    );
}

use bevy::diagnostic::{
    Diagnostic, DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::ecs::system::{Res, SystemParam};
use bevy_egui::egui;

use super::WriteParams;

#[derive(SystemParam)]
pub struct WriteDiagnosticsParams<'w> {
    diagnostics: Res<'w, DiagnosticsStore>,
}

impl WriteParams for WriteDiagnosticsParams<'_> {
    fn title(&self) -> String { "Diagnostics".into() }

    fn default_open() -> bool { false }

    fn write(&mut self, ui: &mut egui::Ui) {
        if let Some(fps) =
            self.diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(Diagnostic::smoothed)
        {
            ui.label(format!("FPS: {fps}"));
        }

        if let Some(entities) = self
            .diagnostics
            .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
            .and_then(Diagnostic::value)
        {
            ui.label(format!("Entities: {entities}"));
        }
    }
}

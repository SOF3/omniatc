use bevy::ecs::system::{Res, SystemParam};
use bevy_egui::egui;
use omniatc::level::score;

use super::WriteParams;

#[derive(SystemParam)]
pub struct WriteScoreParams<'w> {
    score: Res<'w, score::Stats>,
}

impl WriteParams for WriteScoreParams<'_> {
    fn title(&self) -> String { format!("Score: {}", self.score.total.0) }

    fn default_open() -> bool { true }

    fn write(&mut self, ui: &mut egui::Ui) {
        ui.label(format!(
            "Arrivals completed: {}",
            self.score.num_runway_arrivals + self.score.num_apron_arrivals
        ));
        ui.label(format!("Departures completed: {}", self.score.num_departures));
    }
}

use bevy::ecs::system::{Local, Res, ResMut, SystemParam};
use bevy::time::{self, Time};
use bevy_egui::egui;

use super::WriteParams;
use crate::input;

#[derive(SystemParam)]
pub struct WriteTimeParams<'w, 's> {
    time:          ResMut<'w, Time<time::Virtual>>,
    regular_speed: Local<'s, Option<f32>>,
    hotkeys:       Res<'w, input::Hotkeys>,
    paused:        Local<'s, bool>,
}

impl WriteParams for WriteTimeParams<'_, '_> {
    fn title(&self) -> String { "Time".into() }

    fn default_open() -> bool { true }

    fn write(&mut self, ui: &mut egui::Ui) {
        // NOTE: do not allow values that are too high to avoid significant simulation instability.
        const FAST_FORWARD_SPEED: f32 = 25.0;

        let elapsed = self.time.elapsed().as_secs();

        ui.label(format!(
            "Time: {hours}:{minutes:02}:{seconds:02}",
            hours = elapsed / 3600,
            minutes = (elapsed / 60) % 60,
            seconds = elapsed % 60
        ));

        if self.hotkeys.toggle_pause {
            *self.paused = !*self.paused;
        }

        let desired_speed = ui
            .horizontal(|ui| {
                let regular_speed = self.regular_speed.get_or_insert(1.0);

                ui.add(
                    egui::Slider::new(regular_speed, 0. ..=20.).prefix("Game speed: ").suffix("x"),
                );

                if self.hotkeys.reset_speed {
                    *regular_speed = 1.0;
                }

                if self.hotkeys.fast_forward {
                    ui.label(format!("{FAST_FORWARD_SPEED}x"));
                    FAST_FORWARD_SPEED
                } else if *self.paused {
                    ui.label("Paused");
                    0.0
                } else {
                    *regular_speed
                }
            })
            .inner;

        #[expect(clippy::float_cmp, reason = "float is exactly equal if nobody touched it")]
        if self.time.relative_speed() != desired_speed {
            self.time.set_relative_speed(desired_speed);
            if desired_speed > 0. {
                self.time.unpause();
            } else {
                self.time.pause();
            }
        }
    }
}

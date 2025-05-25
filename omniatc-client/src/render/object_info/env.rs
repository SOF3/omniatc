use bevy::ecs::query::QueryData;
use bevy_egui::egui;
use omniatc::level::{plane, wake, wind};
use omniatc::math::Sign;
use omniatc::units::Angle;

use super::Writer;

#[derive(QueryData)]
pub struct ObjectQuery {
    wake:  Option<&'static wake::Detector>,
    wind:  Option<&'static wind::Detector>,
    plane: Option<&'static plane::Control>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = ();

    fn title() -> &'static str { "Environment" }

    fn should_show(this: &Self::Item<'_>) -> bool { this.wake.is_some() || this.wind.is_some() }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, (): &mut Self::SystemParams<'_, '_>) {
        if let Some(wake) = this.wake {
            ui.label(format!("Wake: {:.2}", f64::from(wake.last_detected.0) / 60000.));
        }
        if let Some(wind) = this.wind {
            let wind = wind.last_computed;

            let magnitude = wind.magnitude_exact().into_knots();
            ui.label(format!(
                "Wind: {magnitude:.1} kt from {:.0}\u{b0}",
                wind.heading().opposite().degrees()
            ));

            if let Some(plane) = this.plane {
                let tail_wind = wind.project_onto_dir(plane.heading.into_dir2());
                let cross_wind = wind.project_onto_dir((plane.heading + Angle::RIGHT).into_dir2());

                match tail_wind.sign() {
                    Sign::Negative => {
                        ui.small(format!("Head wind: {:.1} kt", -tail_wind.into_knots()));
                    }
                    Sign::Zero => {}
                    Sign::Positive => {
                        ui.small(format!("Tail wind: {:.1} kt", tail_wind.into_knots()));
                    }
                }

                match cross_wind.sign() {
                    Sign::Negative => {
                        ui.small(format!(
                            "Cross wind from right: {:.1} kt",
                            -cross_wind.into_knots()
                        ));
                    }
                    Sign::Zero => {}
                    Sign::Positive => {
                        ui.small(format!(
                            "Cross wind from left: {:.1} kt",
                            cross_wind.into_knots()
                        ));
                    }
                }
            }
        }
    }
}

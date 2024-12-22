use bevy::app::{App, Plugin};
use bevy::color::{Color, Mix};
use bevy::prelude::Resource;

use crate::math::{LengthUnit, SpeedUnit};

mod render;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<DisplayConfig>();

        app.add_plugins(render::Plug);
    }
}

#[derive(Resource)]
pub struct DisplayConfig {
    /// Size of plane sprites.
    pub plane_sprite_size: f32,
    pub color_scheme:      ColorScheme,

    /// Size of object labels.
    pub label_size:     f32,
    /// Structure of object labels.
    pub label_elements: Vec<LabelLine>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            plane_sprite_size: 1.,
            color_scheme:      ColorScheme::Mixed {
                a:      Box::new(ColorScheme::Destination {
                    departure: vec![Color::srgb(1., 0., 0.)],
                    arrival:   vec![Color::srgb(0., 1., 0.)],
                    ferry:     vec![Color::srgb(0., 0., 1.)],
                }),
                b:      Box::new(ColorScheme::Altitude(ColorScale {
                    pieces: vec![Color::WHITE, Color::srgb(0.2, 0.2, 0.2)],
                })),
                factor: 0.5,
            },
            label_size:        0.5,
            label_elements:    vec![
                LabelLine { elements: vec![LabelElement::Name] },
                LabelLine {
                    elements: vec![
                        LabelElement::CurrentHeading,
                        LabelElement::TargetHeading
                            .surround_if_filled(Some(LabelElement::Const(" -> ".into())), None),
                    ],
                },
                LabelLine {
                    elements: vec![
                        LabelElement::CurrentIndicatedAirspeed(SpeedUnit::Knot),
                        LabelElement::TargetAirspeed(SpeedUnit::Knot)
                            .surround_if_filled(Some(LabelElement::Const(" -> ".into())), None),
                    ],
                },
                LabelLine {
                    elements: vec![
                        LabelElement::CurrentAltitude(LengthUnit::Feet),
                        LabelElement::TargetAltitude(LengthUnit::Feet)
                            .surround_if_filled(Some(LabelElement::Const(" -> ".into())), None),
                    ],
                },
            ],
        }
    }
}

/// Color scheme for objects.
pub enum ColorScheme {
    /// Colors for departures and arrivals from/to different aerodromes have different colors.
    Destination {
        /// A departure from aerodrome #n uses color `departure[n.min(departure.len() - 1)]`.
        departure: Vec<Color>,
        /// An arrival to aerodrome #n uses color `arrival[n.min(arrival.len() - 1)]`.
        arrival:   Vec<Color>,
        /// A ferry to aerodrome #n uses color `ferry[n.min(ferry.len() - 1)]`.
        ferry:     Vec<Color>,
    },
    /// Color changes as the altitude increases.
    Altitude(ColorScale),
    /// Mixes two color schemes together.
    Mixed {
        /// The first color scheme.
        a:      Box<ColorScheme>,
        /// The second color scheme.
        b:      Box<ColorScheme>,
        /// The mixing factor.
        ///
        /// `0.0` uses `a` completely,
        /// `1.0` uses `b` completely.
        /// `0.5` is the middle of the two.
        factor: f32,
    },
}

/// A linear color scale for values from 0 to 1.
pub struct ColorScale {
    /// Evenly separated interpolation points for the color scale.
    /// Must have at least two elements.
    pub pieces: Vec<Color>,
}

impl ColorScale {
    /// Resolves the color for the given value.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // checked: [0, 1] * [0, n-1] -> [0, n-1]
    pub fn get(&self, value: f32) -> Color {
        let segments = self.pieces.len() - 1;
        #[allow(clippy::cast_precision_loss)] // assuming self.pieces is reasonably small
        let position = value.clamp(0., 1.) * (segments as f32);
        let left = &self.pieces[position.floor() as usize];
        let right = &self.pieces[position.ceil() as usize];
        left.mix(right, position.fract())
    }
}

/// A line of object label.
pub struct LabelLine {
    /// Elements on the line, from left to right.
    pub elements: Vec<LabelElement>,
}

/// An element type to be written to a label.
pub enum LabelElement {
    /// A constant string.
    Const(String),

    /// Conditional labels depending on whether `main` is empty.
    IfEmpty {
        main:             Box<LabelElement>,
        prefix_if_filled: Option<Box<LabelElement>>,
        suffix_if_filled: Option<Box<LabelElement>>,
        if_empty:         Option<Box<LabelElement>>,
    },

    /// Name of the object.
    Name,
    /// Current indicated airspeed of the object. Empty when on ground.
    CurrentIndicatedAirspeed(SpeedUnit),
    /// Current ground speed of the object.
    CurrentGroundSpeed(SpeedUnit),
    /// Current altitude of the object.
    CurrentAltitude(LengthUnit),

    /// Current heading, if available. Empty for objects without a heading.
    CurrentHeading,

    /// Target airspeed of the object, if it is navigating.
    /// Empty if airspeed is uncontrolled.
    TargetAirspeed(SpeedUnit),
    /// Target altitude of the object, if it is navigating.
    /// Empty if altitude is uncontrolled.
    TargetAltitude(LengthUnit),
    /// Target climb rate of the object, if it is navigating.
    /// Empty if climb rate is uncontrolled.
    TargetClimbRate(LengthUnit),
    /// Target heading of the object, if it is navigating.
    /// Empty if direction is uncontrolled.
    TargetHeading,
}

impl LabelElement {
    pub fn surround_if_filled(self, prefix: Option<Self>, suffix: Option<Self>) -> Self {
        Self::IfEmpty {
            main:             Box::new(self),
            prefix_if_filled: prefix.map(Box::new),
            suffix_if_filled: suffix.map(Box::new),
            if_empty:         None,
        }
    }

    pub fn replace_if_empty(self, then: Self) -> Self {
        Self::IfEmpty {
            main:             Box::new(self),
            prefix_if_filled: None,
            suffix_if_filled: None,
            if_empty:         Some(Box::new(then)),
        }
    }
}

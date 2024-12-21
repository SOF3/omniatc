use bevy::app::{App, Plugin};
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

    /// Size of object labels.
    pub label_size:     f32,
    /// Structure of object labels.
    pub label_elements: Vec<LabelLine>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            plane_sprite_size: 1.,
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

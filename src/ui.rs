use bevy::app::{self, App, Plugin};
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{in_state, AppExtStates, IntoSystemSetConfigs, States, SystemSet};
use omniatc_core::units::Position;
use strum::IntoEnumIterator;

mod billboard;
mod camera;
mod clock;
mod ground;
mod message;
mod object;
mod runway;
mod shapes;
mod track;
mod waypoint;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_state::<InputState>();

        app.add_plugins(shapes::Plug);
        app.add_plugins(camera::Plug);
        app.add_plugins(message::Plug);
        app.add_plugins(clock::Plug);
        app.add_plugins(billboard::Plug);
        app.add_plugins(ground::Plug);
        app.add_plugins(object::Plug);
        app.add_plugins(runway::Plug);
        app.add_plugins(waypoint::Plug);
        app.add_plugins(track::Plug);

        app.configure_sets(app::Update, SystemSets::RenderSpawn.before(SystemSets::RenderMove));
        app.configure_sets(
            app::Update,
            SystemSets::RenderMove.ambiguous_with(SystemSets::RenderMove),
        );
        app.configure_sets(
            app::Update,
            (SystemSets::RenderSpawn, SystemSets::RenderMove).in_set(SystemSets::RenderAll),
        );

        for (i, state) in InputState::iter().enumerate() {
            app.configure_sets(app::Update, state.in_set(SystemSets::Input));
            app.configure_sets(app::Update, state.run_if(in_state(state)));

            for other_state in InputState::iter().skip(i + 1) {
                app.configure_sets(app::Update, state.ambiguous_with(other_state));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum SystemSets {
    Input,
    RenderAll,
    RenderSpawn,
    RenderMove,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States, SystemSet, strum::EnumIter)]
pub enum InputState {
    /// Root menu.
    #[default]
    Root,
    /// Searching objects by name.
    ObjectSearch,
    /// Operates on a specific object.
    ObjectAction,
    /// Set object speed.
    ObjectSetSpeed,
    /// Set object heading.
    ObjectSetHeading,
    /// Set object altitude.
    ObjectSetAltitude,
}

/// Renderable layers.
///
/// The first item is the lowest layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumCount)]
#[repr(u32)]
pub enum Zorder {
    Terrain,
    GroundSegmentCenterline,
    RunwayStrip,
    Localizer,
    LocalizerGlidePoint,
    ObjectTrack,
    Waypoint,
    WaypointLabel,
    Object,
    ObjectSeparation,
    ObjectLabel,
    ScaleRuler,
    ScaleRulerLabel,
    MessageText,
}

impl Zorder {
    #[expect(clippy::cast_precision_loss)] // the number of items is small
    pub const fn into_z(self) -> f32 {
        (self as u32 as f32) / (<Self as strum::EnumCount>::COUNT as f32)
    }

    pub fn pos2_to_translation(self, position: Position<Vec2>) -> Vec3 {
        (position.get(), self.into_z()).into()
    }

    pub fn pos3_to_translation(self, position: Position<Vec3>) -> Vec3 {
        self.pos2_to_translation(position.horizontal())
    }
}

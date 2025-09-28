use bevy::app::{App, Plugin};
use bevy::ecs::resource::Resource;
use bevy::math::{Vec2, Vec3};
use bevy::transform::components::Transform;
use math::{Length, Position};

mod aerodrome;
pub mod camera;
pub mod object;
pub mod pick;
mod runway;
mod wake;
mod waypoint;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            camera::Plug,
            object::Plug,
            pick::Plug,
            waypoint::Plug,
            runway::Plug,
            aerodrome::Plug,
            wake::Plug,
        ));
    }
}

/// Whether 2D rendering is used.
#[derive(Resource)]
pub struct Active(pub bool);

/// Renderable layers.
///
/// The first item is the lowest layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumCount)]
#[repr(u16)]
pub enum Zorder {
    Terrain,
    GroundSegmentBackground,
    GroundSegmentCenterline,
    RunwayStrip,
    Localizer,
    LocalizerGlidePoint,
    GroundSegmentLabel,
    ObjectTrack,
    WaypointSprite,
    WaypointLabel,
    WakeOverlay,
    ObjectSprite,
    ObjectSeparationRing,
    ObjectVector,
    ObjectLabel,
    RoutePresetPreview,
    ObjectTrackPreview,
    PossibleGroundPathPreview,
    ScaleRuler,
    ScaleRulerLabel,
}

impl Zorder {
    #[expect(clippy::cast_precision_loss)] // the number of items is small
    pub fn into_z(self) -> f32 {
        f32::from(self as u16) / (<Self as strum::EnumCount>::COUNT as f32) / 1024.0
    }

    pub fn local_translation(self) -> Transform {
        Transform::from_translation(Vec3::new(0., 0., self.into_z()))
    }

    pub fn pos2_to_translation(self, position: Position<Vec2>) -> Vec3 {
        (position.get(), self.into_z()).into()
    }

    pub fn pos3_to_translation(self, position: Position<Vec3>) -> Vec3 {
        self.pos2_to_translation(position.horizontal())
    }

    pub fn dist2_to_translation(self, distance: Length<Vec2>) -> Vec3 {
        (distance.0, self.into_z()).into()
    }

    pub fn dist3_to_translation(self, distance: Length<Vec3>) -> Vec3 {
        self.dist2_to_translation(distance.horizontal())
    }

    pub fn base_translation(position: Position<Vec3>) -> Vec3 { position.get().with_z(0.) }
}

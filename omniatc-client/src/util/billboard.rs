use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::Query;
use bevy::math::{Vec2, Vec3};
use bevy::sprite::{Anchor, Text2d};
use bevy::transform::components::Transform;
use math::Length;

use crate::render::{self};
use crate::util::ActiveCamera2d;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            (maintain_scale_system, maintain_rot_system, translate_label_system)
                .in_set(render::SystemSets::Update),
        );
    }
}

/// Entities with this component always have the same scale regardless of camera zoom.
#[derive(Component)]
#[require(Transform)]
pub struct MaintainScale {
    pub size: f32,
}

/// Entities with this component always have the same orientation regardless of camera rotation.
#[derive(Component)]
#[require(Transform)]
pub struct MaintainRotation;

/// Entities with this component always have the same translation from the parent transform regardless of camera orientation and zoom.
#[derive(Component)]
#[require(Text2d)]
pub struct Label {
    /// Further offsets the label from the parent in real-world coordinates.
    pub offset:   Length<Vec2>,
    /// Distance to displace the label from the offset position in the anchor direction,
    /// in screen coordinates.
    pub distance: f32,
}

fn translate_label_system(
    camera: ActiveCamera2d,
    mut query: Query<(&Label, &Anchor, &mut Transform)>,
) {
    query.iter_mut().for_each(|(bb, anchor, mut tf)| {
        let offset = camera.affine_transform(anchor.as_vec());
        tf.translation = (bb.offset.0 + (offset * bb.distance), tf.translation.z).into();
    });
}

fn maintain_scale_system(
    camera: ActiveCamera2d,
    mut query: Query<(&MaintainScale, &mut Transform)>,
) {
    query.iter_mut().for_each(|(maintain, mut tf)| {
        let scale = camera.scale() * maintain.size;
        tf.scale = Vec3::new(scale, scale, 1.0);
    });
}

fn maintain_rot_system(
    camera: ActiveCamera2d,
    mut query: Query<(&MaintainRotation, &mut Transform)>,
) {
    query.iter_mut().for_each(|(MaintainRotation, mut tf)| {
        tf.rotation = camera.rotation();
    });
}

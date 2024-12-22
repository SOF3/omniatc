use bevy::app::{self, App, Plugin};
use bevy::math::{Vec3, Vec3Swizzles};
use bevy::prelude::{
    Camera2d, Component, GlobalTransform, IntoSystemConfigs, Query, Single, Transform, With,
};
use bevy::sprite::Anchor;
use bevy::text::Text2d;

use super::SystemSets;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            (maintain_tf_system, translate_label_system).in_set(SystemSets::RenderMove),
        );
    }
}

/// Entities with this component always have the same scale regardless of camera zoom.
#[derive(Component)]
pub struct MaintainScale {
    pub size: f32,
}

/// Entities with this component always have the same orientation regardless of camera rotation.
#[derive(Component)]
pub struct MaintainRotation;

/// Entities with this component always have the same translation from the parent transform regardless of camera orientation and zoom.
#[derive(Component)]
#[require(Text2d)]
pub struct Label {
    pub distance: f32,
}

fn translate_label_system(
    camera: Single<&GlobalTransform, With<Camera2d>>,
    mut query: Query<(&Label, &Anchor, &mut Transform)>,
) {
    let camera = *camera;

    query.iter_mut().for_each(|(bb, anchor, mut tf)| {
        let offset = camera.affine().matrix3 * Vec3::from((anchor.as_vec(), 0.));
        tf.translation = ((offset * bb.distance).xy(), tf.translation.z).into();
    });
}

fn maintain_tf_system(
    camera: Single<&GlobalTransform, With<Camera2d>>,
    mut query: Query<(&MaintainScale, &mut Transform)>,
) {
    let camera = *camera;

    query.iter_mut().for_each(|(maintain, mut tf)| {
        tf.rotation = camera.rotation();
        tf.scale = camera.scale() * maintain.size;
    });
}

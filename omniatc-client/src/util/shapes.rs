use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, ResMut, Single};
use bevy::math::primitives::{Circle, Rectangle};
use bevy::math::Vec2;
use bevy::render::mesh::{Mesh, Mesh2d};
use bevy::transform::components::{GlobalTransform, Transform};
use omniatc_core::units::Distance;

use crate::render;
use crate::render::twodim::Zorder;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Meshes>();
        app.add_systems(app::Startup, Meshes::init_system);
        app.add_systems(app::Update, maintain_thickness_system.in_set(render::SystemSets::Update));
    }
}

#[derive(Default, Resource)]
pub struct Meshes {
    square: Option<Handle<Mesh>>,
    circle: Option<Handle<Mesh>>,
}

impl Meshes {
    pub fn init_system(mut store: ResMut<Self>, mut meshes: ResMut<Assets<Mesh>>) {
        store.square = Some(meshes.add(Rectangle::new(1., 1.)));
        store.circle = Some(meshes.add(Circle::new(1.)));
    }

    pub fn square(&self) -> &Handle<Mesh> {
        self.square.as_ref().expect("initialized during startup")
    }

    pub fn circle(&self) -> &Handle<Mesh> {
        self.circle.as_ref().expect("initialized during startup")
    }

    pub fn line(&self, thickness: f32, zorder: Zorder) -> impl Bundle {
        (Mesh2d(self.square().clone()), square_line_transform(zorder), MaintainThickness(thickness))
    }
}

fn square_line_transform(zorder: Zorder) -> Transform {
    let mut tf = Transform::default();
    tf.translation.z = zorder.into_z();
    tf
}

pub fn set_square_line_transform(tf: &mut Transform, length: Distance<Vec2>) {
    let translation = (length / 2.).0;
    tf.translation.x = translation.x;
    tf.translation.y = translation.y;
    tf.rotation = length.heading().into_rotation_quat();

    // X = thickness, Y = end-to-end
    tf.scale.y = length.magnitude_exact().0;
}

#[derive(Component)]
pub struct MaintainThickness(pub f32);

fn maintain_thickness_system(
    mut query: Query<(&MaintainThickness, &mut Transform)>,
    camera: Single<&GlobalTransform, With<Camera2d>>,
) {
    query.iter_mut().for_each(|(thickness, mut tf)| {
        tf.scale.x = thickness.0 * camera.scale().y;
    });
}

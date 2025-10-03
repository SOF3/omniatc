use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, ResMut};
use bevy::math::Vec2;
use bevy::math::primitives::{Circle, Rectangle};
use bevy::mesh::{Mesh, Mesh2d};
use bevy::transform::components::Transform;
use math::{Length, Position};

use crate::render::twodim::Zorder;
use crate::render::{self};
use crate::util::ActiveCamera2d;

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

    pub fn line_from_to(
        &self,
        thickness: f32,
        zorder: Zorder,
        from: Position<Vec2>,
        to: Position<Vec2>,
        camera: &ActiveCamera2d,
    ) -> impl Bundle {
        let mut tf = square_line_transform(zorder);
        set_square_line_transform_relative(&mut tf, from.0, to.0);
        tf.scale.x = camera.scale() * thickness;
        (Mesh2d(self.square().clone()), tf, MaintainThickness(thickness))
    }
}

fn square_line_transform(zorder: Zorder) -> Transform {
    let mut tf = Transform::default();
    tf.translation.z = zorder.into_z();
    tf
}

pub fn set_square_line_transform(tf: &mut Transform, start: Position<Vec2>, end: Position<Vec2>) {
    set_square_line_transform_relative(tf, start.0, end.0);
}

pub fn set_square_line_transform_relative(
    tf: &mut Transform,
    start: Length<Vec2>,
    end: Length<Vec2>,
) {
    let midpt = start.lerp(end, 0.5);
    let translation = midpt.0;
    tf.translation.x = translation.x;
    tf.translation.y = translation.y;

    let length = end - start;
    tf.rotation = length.heading().into_rotation_quat();

    // X = thickness, Y = end-to-end
    tf.scale.y = length.magnitude_exact().0;
}

#[derive(Component)]
pub struct MaintainThickness(pub f32);

fn maintain_thickness_system(
    mut query: Query<(&MaintainThickness, &mut Transform)>,
    camera: ActiveCamera2d,
) {
    query.iter_mut().for_each(|(thickness, mut tf)| {
        tf.scale.x = camera.scale() * thickness.0;
    });
}

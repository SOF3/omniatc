use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Assets};
use bevy::ecs::system::SystemParam;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{
    Bundle, Camera2d, Component, GlobalTransform, IntoSystemConfigs, Mesh, Mesh2d, Query,
    Rectangle, Res, ResMut, Resource, Single, Transform, With,
};
use omniatc_core::units::Position;

use super::{SystemSets, Zorder};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Meshes>();
        app.add_systems(app::Startup, setup_system);
        app.add_systems(app::Update, maintain_line_system.after(SystemSets::RenderMove));
    }
}

#[derive(Resource, Default)]
struct Meshes {
    square: Option<asset::Handle<Mesh>>,
}

fn setup_system(mut store: ResMut<Meshes>, mut meshes: ResMut<Assets<Mesh>>) {
    store.square = Some(meshes.add(Rectangle::new(1., 1.)));
}

#[derive(SystemParam)]
pub struct DrawLine<'w> {
    meshes: Res<'w, Meshes>,
    camera: Single<'w, &'static GlobalTransform, With<Camera2d>>,
}

#[derive(Component)]
pub struct MaintainLine {
    thickness: f32,
}

impl DrawLine<'_> {
    pub fn bundle(
        &self,
        start: Position<Vec2>,
        end: Position<Vec2>,
        thickness: f32,
        zorder: Zorder,
    ) -> impl Bundle {
        (
            Mesh2d(self.meshes.square.clone().expect("initialized at startup")),
            make_line_transform(start, end, thickness, zorder.into_z(), *self.camera),
            MaintainLine { thickness },
        )
    }
}

fn make_line_transform(
    start: Position<Vec2>,
    end: Position<Vec2>,
    thickness: f32,
    z: f32,
    global_tf: &GlobalTransform,
) -> Transform {
    Transform {
        translation: (start.midpoint(end).get(), z).into(),
        rotation:    (end - start).heading().into_rotation_quat(),
        scale:       Vec3::new(thickness * global_tf.scale().x, start.distance_exact(end).0, 1.),
    }
}

fn maintain_line_system(
    mut line: Query<(&MaintainLine, &mut Transform)>,
    global_tf: Single<&'static GlobalTransform, With<Camera2d>>,
) {
    line.iter_mut().for_each(|(line, mut tf)| {
        tf.scale.x = line.thickness * global_tf.scale().x;
    });
}

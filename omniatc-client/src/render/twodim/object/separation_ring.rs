use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Assets};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::math::primitives::Annulus;
use bevy::render::mesh::Mesh;
use bevy::transform::components::GlobalTransform;
use omniatc_core::units::Distance;

use super::Conf;
use crate::config;

pub(super) struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeparationRingMesh>();
        app.add_systems(app::Startup, init_system);
        app.add_systems(app::Update, maintain_thickness_system);
    }
}

#[derive(Resource, Default)]
pub(super) struct SeparationRingMesh {
    handle:    Option<asset::Handle<Mesh>>,
    radius:    Distance<f32>,
    thickness: f32,
}

fn init_system(mut handle: ResMut<SeparationRingMesh>, mut assets: ResMut<Assets<Mesh>>) {
    handle.handle = Some(assets.add(Annulus::new(1.4, 1.5)));
}

fn maintain_thickness_system(
    handle: Res<SeparationRingMesh>,
    mut assets: ResMut<Assets<Mesh>>,
    conf: config::Read<Conf>,
    camera_query: Query<&GlobalTransform, With<Camera2d>>,
) {
    #[expect(clippy::float_cmp)] // float is exactly equal if config is unchanged
    if conf.separation_ring_radius == handle.radius
        && conf.separation_ring_thickness == handle.thickness
    {
        return;
    }

    let asset = assets
        .get_mut(handle.handle.as_ref().expect("initialized during startup"))
        .expect("strong handle stored in resource");

    let radius = conf.separation_ring_radius.0;

    let camera_scale = match camera_query.iter().next() {
        Some(global_tf) => global_tf.scale().x,
        None => 1.,
    };
    let thickness_scaled = (conf.separation_ring_thickness * camera_scale).min(radius);

    *asset = Annulus::new(radius - thickness_scaled, radius).into();
}

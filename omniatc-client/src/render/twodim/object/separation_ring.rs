use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Assets};
use bevy::color::Color;
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::math::primitives::Annulus;
use bevy::render::mesh::{Mesh, Mesh2d};
use bevy::render::view::Visibility;
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::GlobalTransform;
use omniatc::level::object;
use omniatc::try_log;
use omniatc::units::Distance;
use omniatc::util::EnumScheduleConfig;

use super::{ColorTheme, Conf, SetColorThemeSystemSet};
use crate::render::twodim::Zorder;
use crate::{config, render};

pub(super) struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeparationRingMesh>();
        app.add_systems(app::Startup, init_system);
        app.add_systems(app::Update, maintain_thickness_system.in_set(render::SystemSets::Update));
        app.add_systems(
            app::Update,
            maintain_color_system
                .in_set(render::SystemSets::Update)
                .after_all::<SetColorThemeSystemSet>(),
        );
        app.add_systems(
            app::Update,
            maintain_visible_system
                .in_set(render::SystemSets::Update),
        );
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

#[derive(SystemParam)]
pub(super) struct SpawnSubsystemParam<'w, 's> {
    commands:  Commands<'w, 's>,
    mesh:      Res<'w, SeparationRingMesh>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
}

#[derive(Component)]
#[relationship(relationship_target = HasRing)]
struct IsRingOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsRingOf, linked_spawn)]
struct HasRing(Entity);

pub(super) fn spawn_subsystem(plane_entity: Entity, p: &mut SpawnSubsystemParam) {
    let material = p.materials.add(ColorMaterial { color: Color::WHITE, ..Default::default() });

    p.commands.spawn((
        ChildOf(plane_entity),
        IsRingOf(plane_entity),
        Zorder::ObjectSeparationRing.local_translation(),
        Mesh2d(p.mesh.handle.clone().expect("initialized during startup")),
        MeshMaterial2d(material),
    ));
}

fn maintain_color_system(
    object_query: Query<(&ColorTheme, &HasRing)>,
    ring_query: Query<&MeshMaterial2d<ColorMaterial>>,
    mut material_assets: ResMut<Assets<ColorMaterial>>,
) {
    for (color, &HasRing(ring_entity)) in object_query {
        let material_handle = try_log!(ring_query.get(ring_entity), expect "HasRing must reference valid ring viewable" or continue);
        let material = material_assets
            .get_mut(&material_handle.0)
            .expect("asset from strong handle must exist");
        material.color = color.ring;
    }
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

fn maintain_visible_system(
    object_query: Query<(Option<&object::Airborne>, &HasRing)>,
    mut ring_query: Query<&mut Visibility, With<IsRingOf>>,
) {
    for (airborne, &HasRing(ring_entity)) in object_query {
        let visible = if airborne.is_some() { Visibility::Inherited } else { Visibility::Hidden };

        let mut ring_visibility = try_log!(
            ring_query.get_mut(ring_entity),
            expect "HasRing must reference valid ring viewable" or continue
        );
        *ring_visibility = visible;
    }
}

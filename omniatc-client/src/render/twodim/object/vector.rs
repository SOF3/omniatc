use bevy::app::{self, App, Plugin};
use bevy::asset::Assets;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::Transform;
use bevy_mod_config::ReadConfig;
use math::Length;
use omniatc::level::object::Object;
use omniatc::try_log;
use omniatc::util::EnumScheduleConfig;

use super::{ColorTheme, Conf, SetColorThemeSystemSet};
use crate::render;
use crate::render::twodim::Zorder;
use crate::util::shapes;

pub(super) struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            maintain_color_system
                .in_set(render::SystemSets::Update)
                .after_all::<SetColorThemeSystemSet>(),
        );
        app.add_systems(app::Update, maintain_length_system.in_set(render::SystemSets::Update));
    }
}

#[derive(SystemParam)]
pub(super) struct SpawnSubsystemParam<'w, 's> {
    commands:  Commands<'w, 's>,
    meshes:    Res<'w, shapes::Meshes>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
    conf:      ReadConfig<'w, 's, Conf>,
}

#[derive(Component)]
#[relationship(relationship_target = HasVector)]
struct IsVectorOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsVectorOf, linked_spawn)]
struct HasVector(Entity);

pub(super) fn spawn_subsystem(plane_entity: Entity, p: &mut SpawnSubsystemParam) {
    let material = p.materials.add(ColorMaterial { color: Color::WHITE, ..Default::default() });

    p.commands.spawn((
        ChildOf(plane_entity),
        IsVectorOf(plane_entity),
        p.meshes.line(p.conf.read().vector.thickness, Zorder::ObjectVector),
        MeshMaterial2d(material),
    ));
}

fn maintain_color_system(
    object_query: Query<(&ColorTheme, &HasVector)>,
    vector_query: Query<&MeshMaterial2d<ColorMaterial>>,
    mut material_assets: ResMut<Assets<ColorMaterial>>,
) {
    for (color, &HasVector(vector_entity)) in object_query {
        let material_handle = try_log!(vector_query.get(vector_entity), expect "HasVector must reference valid vector viewable" or continue);
        let material = material_assets
            .get_mut(&material_handle.0)
            .expect("asset from strong handle must exist");
        material.color = color.vector;
    }
}

fn maintain_length_system(
    conf: ReadConfig<Conf>,
    object_query: Query<(&Object, &HasVector)>,
    mut vector_query: Query<&mut Transform, With<IsVectorOf>>,
) {
    let conf = conf.read();

    for (object, &HasVector(vector_entity)) in object_query {
        let vector_dist = object.ground_speed.horizontal() * conf.vector.lookahead_time;
        let mut transform = try_log!(vector_query.get_mut(vector_entity), expect "HasVector must reference valid vector viewable" or continue);
        shapes::set_square_line_transform_relative(&mut transform, Length::ZERO, vector_dist);
    }
}

use bevy::app::{self, App, Plugin};
use bevy::asset::Assets;
use bevy::color::Mix;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::render::mesh::Mesh2d;
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::Transform;
use omniatc_core::level::object;
use omniatc_core::try_log;
use omniatc_core::util::{manage_entity_vec, EnumScheduleConfig};

use super::{Conf, SetColorThemeSystemSet};
use crate::render::object_info;
use crate::render::twodim::Zorder;
use crate::util::{billboard, shapes};
use crate::{config, render};

pub(super) struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            respawn_system.in_set(render::SystemSets::Update).after_all::<SetColorThemeSystemSet>(),
        );
    }
}

#[derive(Component)]
#[relationship(relationship_target = PointList)]
struct IsPointOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsPointOf, linked_spawn)]
struct PointList(Vec<Entity>);

fn respawn_system(
    object_query: Query<(Entity, &object::Track, Option<&PointList>)>,
    mut point_query: Query<(&mut Transform, &MeshMaterial2d<ColorMaterial>), With<IsPointOf>>,
    shapes: Res<shapes::Meshes>,
    mut material_assets: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
    conf: config::Read<Conf>,
    current_object: Res<object_info::CurrentObject>,
) {
    for (object_entity, track, point_list) in object_query {
        let point_data = track
            .log
            .iter()
            .rev()
            .take(if current_object.0 == Some(object_entity) {
                track.log.len()
            } else {
                conf.track_normal_max_points as usize
            })
            .map(|&pos| {
                let color = conf.track_point_base_color.mix(
                    &conf.track_point_top_color,
                    pos.altitude().ratio_between(
                        conf.track_point_base_altitude,
                        conf.track_point_top_altitude,
                    ),
                );

                (Zorder::ObjectTrack.pos3_to_translation(pos), color)
            });

        manage_entity_vec(
            object_entity,
            point_list,
            &mut (point_data, &mut material_assets),
            |_, (point_data, material_assets)| {
                let (translation, color) = point_data.next()?;

                Some((
                    Transform { translation, ..Default::default() },
                    billboard::MaintainScale { size: conf.track_point_size },
                    Mesh2d(shapes.circle().clone()),
                    MeshMaterial2d(material_assets.add(color)),
                ))
            },
            |_, (point_data, material_assets), point_entity| {
                let (translation, color) = point_data.next().ok_or(())?;

                let (mut tf, material_ref) = try_log!(
                    point_query.get_mut(point_entity),
                    expect "PointList must contain valid IsPointOf members"
                    or return Err(())
                );

                tf.translation = translation;
                material_assets
                    .get_mut(&material_ref.0)
                    .expect("strong handle must be valid")
                    .color = color;

                Ok(())
            },
            &mut commands,
        );
    }
}

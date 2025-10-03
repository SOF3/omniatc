use bevy::app::{self, App, Plugin};
use bevy::asset::Assets;
use bevy::color::{Color, Mix};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec3;
use bevy::mesh::Mesh2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::Transform;
use bevy_mod_config::{Config, ReadConfig};
use math::{Position, TROPOPAUSE_ALTITUDE};
use omniatc::QueryTryLog;
use omniatc::level::object;
use omniatc::util::{EnumScheduleConfig, manage_entity_vec};

use super::SetColorThemeSystemSet;
use crate::render;
use crate::render::object_info;
use crate::render::twodim::Zorder;
use crate::util::{billboard, shapes};

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
    conf: ReadConfig<super::Conf>,
    current_object: Res<object_info::CurrentObject>,
) {
    let conf = conf.read();

    for (object_entity, track, point_list) in object_query {
        let point_data = track
            .log
            .iter()
            .rev()
            .take(if current_object.0 == Some(object_entity) {
                track.log.len()
            } else {
                conf.track.normal_max_points as usize
            })
            .map(|&pos| {
                let color = conf.track.point_base_color.mix(
                    &conf.track.point_top_color,
                    pos.altitude().ratio_between(
                        conf.track.point_base_altitude,
                        conf.track.point_top_altitude,
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
                    Transform { translation, scale: Vec3::ZERO, ..Default::default() },
                    billboard::MaintainScale { size: conf.track.point_size },
                    Mesh2d(shapes.circle().clone()),
                    MeshMaterial2d(material_assets.add(color)),
                ))
            },
            |_, (point_data, material_assets), point_entity| {
                let (translation, color) = point_data.next().ok_or(())?;

                let Some((mut tf, material_ref)) = point_query.log_get_mut(point_entity) else {
                    return Err(());
                };

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

#[derive(Config)]
pub(super) struct Conf {
    /// Maximum number of track points for unfocused objects.
    #[config(default = 5, min = 0, max = 100)]
    normal_max_points:   u32,
    /// Size of track points.
    #[config(default = 1.0, min = 0.0, max = 3.0)]
    point_size:          f32,
    /// Color of track points at base altitude.
    #[config(default = Color::srgb(0.8, 0.4, 0.6))]
    point_base_color:    Color,
    /// Base altitude for track point coloring.
    #[config(default = Position::SEA_LEVEL)]
    point_base_altitude: Position<f32>,
    /// Color of track points at top altitude.
    #[config(default = Color::srgb(0.4, 0.8, 0.6))]
    point_top_color:     Color,
    /// Top altitude for track point coloring.
    #[config(default = TROPOPAUSE_ALTITUDE)]
    point_top_altitude:  Position<f32>,
}

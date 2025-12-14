use bevy::asset::Assets;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::mesh::Mesh2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::Transform;
use bevy_mod_config::{self, ReadConfig};
use math::Length;
use omniatc::level::runway::Runway;
use omniatc::level::waypoint::Waypoint;
use omniatc::{QueryTryLog, try_log_return};

use super::Conf;
use crate::render::twodim::Zorder;
use crate::util::{billboard, shapes};

#[derive(SystemParam)]
pub struct UpdateParam<'w, 's> {
    conf:              ReadConfig<'w, 's, Conf>,
    glide_point_query: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static MeshMaterial2d<ColorMaterial>,
            &'static mut billboard::MaintainScale,
        ),
        With<IsGlidePointOf>,
    >,
    materials:         ResMut<'w, Assets<ColorMaterial>>,
    shapes:            Res<'w, shapes::Meshes>,
    commands:          Commands<'w, 's>,
}

impl UpdateParam<'_, '_> {
    pub fn update(
        &mut self,
        runway_entity: Entity,
        waypoint: &Waypoint,
        runway: &Runway,
        list: Option<&PointList>,
        localizer_length: Length<f32>,
    ) {
        let conf = self.conf.read();

        let list = list.map(|list| &list.0[..]).unwrap_or_default();

        let start_altitude = waypoint.position.altitude();
        let end_altitude =
            start_altitude + localizer_length * runway.glide_descent.acute_signed_tan();

        let density = conf.glide_point_density;

        #[expect(clippy::cast_possible_truncation)] // f32 -> i32 for a reasonably small value
        let first_multiple = (start_altitude.amsl() / density + 0.5).ceil() as i32;
        #[expect(clippy::cast_possible_truncation)] // f32 -> i32 for a reasonably small value
        let last_multiple = (end_altitude.amsl() / density).floor() as i32;

        let glide_direction = runway
            .landing_length
            .projected_from_elevation_angle(-runway.glide_descent)
            .normalize_by_vertical(density)
            .horizontal();

        // The difference may not be positive when end_altitude is lower than start_altitude + 0.5 * density
        let num_desired_points = usize::try_from(last_multiple - first_multiple + 1).unwrap_or(0);

        let start_altitude_densities = start_altitude.amsl() / density;

        for point_number in 0..num_desired_points {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_precision_loss,
                clippy::cast_possible_wrap,
                reason = "usize -> i32 -> f32 for a reasonably small value"
            )]
            let altitude_densities = (first_multiple + (point_number as i32)) as f32;
            let point_dist = glide_direction * (altitude_densities - start_altitude_densities);

            if let Some(&point_entity) = list.get(point_number) {
                let Some((mut point_tf, material_handle, mut size)) =
                    self.glide_point_query.log_get_mut(point_entity)
                else {
                    return;
                };

                point_tf.translation = Zorder::LocalizerGlidePoint.dist2_to_translation(point_dist);

                let material = try_log_return!(self.materials.get_mut(&material_handle.0), expect "asset referenced by strong handle must exist");
                material.color = conf.glide_point_color;

                size.size = conf.glide_point_size;
            } else {
                self.commands.spawn((
                    ChildOf(runway_entity),
                    IsGlidePointOf(runway_entity),
                    Transform {
                        translation: Zorder::LocalizerGlidePoint.dist2_to_translation(point_dist),
                        ..Default::default()
                    },
                    Mesh2d(self.shapes.circle().clone()),
                    MeshMaterial2d(self.materials.add(ColorMaterial {
                        color: conf.glide_point_color,
                        ..Default::default()
                    })),
                    billboard::MaintainScale { size: conf.glide_point_size },
                ));
            }
        }

        for &point_entity in list.get(num_desired_points..).into_iter().flatten() {
            self.commands.entity(point_entity).despawn();
        }
    }
}

#[derive(Component)]
#[relationship(relationship_target = PointList)]
pub struct IsGlidePointOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsGlidePointOf, linked_spawn)]
pub struct PointList(Vec<Entity>);

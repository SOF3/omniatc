use bevy::app::{self, App, Plugin};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Query, ResMut, Single};
use bevy::math::primitives::InfinitePlane3d;
use bevy::math::{Vec2, Vec3};
use bevy::render::camera::Camera;
use bevy::transform::components::GlobalTransform;
use bevy::window::Window;
use omniatc::units::Position;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentCursorCamera>();
        app.add_systems(
            app::Update,
            CurrentCursorCamera::update_system.before(ReadCurrentCursorCameraSystemSet),
        );
    }
}

#[derive(Resource, Default)]
pub struct CurrentCursorCamera(pub Option<CurrentCursorCameraValue>);

#[derive(Clone, Copy)]
pub struct CurrentCursorCameraValue {
    pub camera_entity: Entity,
    pub viewport_pos:  Vec2,
    pub world_pos:     Position<Vec2>,
}

impl CurrentCursorCamera {
    fn update_system(
        mut target: ResMut<Self>,
        window: Option<Single<&Window>>,
        camera_query: Query<(Entity, &Camera, &GlobalTransform, Option<&Camera2d>)>,
    ) {
        let Some(window) = window else {
            target.0 = None;
            return;
        };

        target.0 = None;
        if let Some(cursor_pos) = window.cursor_position() {
            for (camera_entity, camera, global_tf, is_2d) in camera_query {
                if let Some(viewport_rect) = camera.logical_viewport_rect() {
                    if viewport_rect.contains(cursor_pos) {
                        let viewport_pos = cursor_pos - viewport_rect.min;

                        if is_2d.is_some() {
                            match camera.viewport_to_world_2d(global_tf, cursor_pos) {
                                Ok(world_pos) => {
                                    target.0 = Some(CurrentCursorCameraValue {
                                        camera_entity,
                                        viewport_pos,
                                        world_pos: Position::new(world_pos),
                                    });
                                }
                                Err(err) => bevy::log::error!(
                                    "convert viewport position to world position: {err:?}"
                                ),
                            }
                        } else {
                            match camera.viewport_to_world(global_tf, cursor_pos) {
                                Ok(ray) => {
                                    let dist = ray.intersect_plane(
                                        Position::from_origin_nm(0., 0.)
                                            .with_altitude(Position::SEA_LEVEL)
                                            .get(),
                                        InfinitePlane3d::new(Vec3::Z),
                                    );
                                    if let Some(dist) = dist.filter(|&dist| dist > 0.) {
                                        let world_pos =
                                            Position::new(ray.get_point(dist)).horizontal();
                                        target.0 = Some(CurrentCursorCameraValue {
                                            camera_entity,
                                            viewport_pos,
                                            world_pos,
                                        });
                                    }
                                }
                                Err(err) => bevy::log::error!(
                                    "convert viewport position to world position: {err:?}"
                                ),
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct ReadCurrentCursorCameraSystemSet;

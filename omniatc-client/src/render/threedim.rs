//! Dummy components for 3D camera support in the future.

use bevy::ecs::component::Component;

#[derive(Component, Debug)]
pub struct UiState;

// TODO support 3D camera
/*let ray = try_log_return!(
    data.camera.viewport_to_world(data.global_tf, cursor_pos),
    expect "viewport should be valid"
);
let dist = ray.intersect_plane(
    Position::from_origin_nm(0., 0.).with_altitude(Position::SEA_LEVEL).get(),
    InfinitePlane3d::new(Vec3::Z),
);
if let Some(dist) = dist.filter(|&dist| dist > 0.) {
    let world_pos = Position::new(ray.get_point(dist)).horizontal();
    target.value = Some(CurrentCursorCameraValue {
        camera_entity: data.camera_entity,
        viewport_pos,
        world_pos,
    });
}
*/

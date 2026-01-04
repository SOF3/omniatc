use bevy::camera::Camera;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy::math::{Rect, Vec3, Vec3Swizzles};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy_egui::egui;
use math::{Angle, Heading};
use ordered_float::{Float, OrderedFloat};

use super::WriteParams;
use crate::input;
use crate::render::twodim;

#[derive(SystemParam)]
pub struct WriteCameraParams<'w, 's> {
    camera_query: Query<
        'w,
        's,
        (&'static Camera, &'static mut Transform, &'static GlobalTransform),
        With<twodim::camera::Layout>,
    >,
    cursor:       Res<'w, input::CursorState>,
    hotkeys:      Res<'w, input::Hotkeys>,
}

fn measure_delta(v: f32, f: impl FnOnce(&mut f32)) -> Option<f32> {
    let mut copy = v;
    f(&mut copy);
    Some(copy - v).filter(|&v| v != 0.0)
}

impl WriteParams for WriteCameraParams<'_, '_> {
    fn title(&self) -> String { "Camera".into() }

    fn default_open() -> bool { false }

    fn write(&mut self, ui: &mut egui::Ui) {
        let mut cameras: Vec<_> = self.camera_query.iter_mut().collect();
        cameras.sort_by_key(|(camera, _, _)| match camera.logical_viewport_rect() {
            Some(Rect { min, .. }) => (OrderedFloat(min.x), OrderedFloat(min.y)),
            None => (OrderedFloat::nan(), OrderedFloat::nan()),
        });
        for (i, (camera, mut tf, global_tf)) in cameras.into_iter().enumerate() {
            if i > 0 {
                ui.separator();
            }

            ui.label(format!(
                "Center: ({:.1}, {:.1})",
                global_tf.translation().x,
                global_tf.translation().y,
            ));

            let degrees_delta = measure_delta(
                Heading::from_vec3(global_tf.rotation().mul_vec3(Vec3::Y)).degrees(),
                |degrees| {
                    ui.add(
                        egui::Slider::new(degrees, 0. ..=360.)
                            .text("Direction (up)")
                            .suffix('\u{b0}'),
                    );
                    if self.hotkeys.north {
                        *degrees = 0.;
                    }
                },
            );
            if let Some(degrees_delta) = degrees_delta {
                tf.rotate_z(-Angle::from_degrees(degrees_delta).0);
            }

            if let Some(viewport_size) = camera.logical_viewport_size() {
                let world_size = viewport_size * global_tf.scale().xy();
                ui.label(format!("Viewport size: {:.2} \u{d7} {:.2}", world_size.x, world_size.y));
            }
        }

        if let Some(cursor) = &self.cursor.value {
            ui.label(format!(
                "Cursor: ({:.1}, {:.1})",
                cursor.world_pos.get().x,
                cursor.world_pos.get().y
            ));
        }
    }
}

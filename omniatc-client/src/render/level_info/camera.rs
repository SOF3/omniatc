use bevy::camera::Camera;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, Single, SystemParam};
use bevy::math::{Rect, Vec3, Vec3Swizzles};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy_egui::egui;
use egui_material_icons::icons;
use math::{Angle, Heading};
use omniatc::level::quest;
use ordered_float::{Float, OrderedFloat};

use super::{RequestHighlightParams, WriteParams};
use crate::input;
use crate::render::{tutorial_popup, twodim};

#[derive(SystemParam)]
pub struct WriteCameraParams<'w, 's> {
    camera_query: Query<
        'w,
        's,
        (&'static Camera, &'static mut Transform, &'static GlobalTransform),
        With<twodim::camera::Layout>,
    >,
    request_rot_highlight: Option<
        Single<
            'w,
            's,
            (),
            (With<tutorial_popup::Focused>, With<quest::highlight::SetCameraRotation>),
        >,
    >,
    request_zoom_highlight: Option<
        Single<'w, 's, (), (With<tutorial_popup::Focused>, With<quest::highlight::SetCameraZoom>)>,
    >,
    cursor:                 Res<'w, input::CursorState>,
    hotkeys:                Res<'w, input::Hotkeys>,
    ui_event_writer:        MessageWriter<'w, quest::UiEvent>,
    hl_params:              RequestHighlightParams<'w>,
}

fn measure_delta(v: f32, f: impl FnOnce(&mut f32)) -> Option<f32> {
    let mut copy = v;
    f(&mut copy);
    Some(copy - v).filter(|&v| v != 0.0)
}

impl WriteParams for WriteCameraParams<'_, '_> {
    fn title(&self) -> String { "Camera".into() }

    fn default_open() -> bool { false }

    fn request_highlight(&self) -> Option<&RequestHighlightParams<'_>> {
        (self.request_rot_highlight.is_some() || self.request_zoom_highlight.is_some())
            .then_some(&self.hl_params)
    }

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
                    let mut frame = egui::Frame::NONE;
                    if self.request_rot_highlight.is_some() {
                        frame = frame.stroke((3.0, egui::Color32::RED));
                    }
                    frame.show(ui, |ui| {
                        ui.add(
                            egui::Slider::new(degrees, 0. ..=360.)
                                .text("Direction (up)")
                                .suffix('\u{b0}'),
                        );
                    });
                    if self.hotkeys.north {
                        *degrees = 0.;
                    }
                },
            );
            if let Some(degrees_delta) = degrees_delta {
                tf.rotate_z(-Angle::from_degrees(degrees_delta).0);
                self.ui_event_writer.write(quest::UiEvent::CameraRotated);
            }

            if let Some(viewport_size) = camera.logical_viewport_size() {
                let world_size = viewport_size * global_tf.scale().xy();
                let mut frame = egui::Frame::NONE;
                if self.request_zoom_highlight.is_some() {
                    frame = frame.stroke((3.0, egui::Color32::RED));
                }
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(icons::ICON_ZOOM_OUT).on_hover_text("Zoom out").clicked() {
                            tf.scale *= 1.5;
                            self.ui_event_writer.write(quest::UiEvent::CameraZoomed);
                        }
                        ui.label(format!(
                            "Viewport size: {:.2} \u{d7} {:.2}",
                            world_size.x, world_size.y
                        ));
                        if ui.button(icons::ICON_ZOOM_IN).on_hover_text("Zoom in").clicked() {
                            tf.scale /= 1.5;
                            self.ui_event_writer.write(quest::UiEvent::CameraZoomed);
                        }
                    });
                });
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

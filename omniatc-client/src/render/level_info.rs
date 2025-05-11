use bevy::app::{App, Plugin};
use bevy::diagnostic::{
    Diagnostic, DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::math::{Rect, Vec3, Vec3Swizzles};
use bevy::render::camera::Camera;
use bevy::time::{self, Time};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy_egui::{egui, EguiContextPass, EguiContexts};
use egui_extras::{Column, TableBuilder};
use omniatc_core::level::object;
use omniatc_core::units::{Angle, Heading};
use ordered_float::{Float, OrderedFloat};
use strum::IntoEnumIterator;

use super::config_editor;
use crate::{input, EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiContextPass, setup_layout_system.in_set(EguiSystemSets::LevelInfo));
    }
}

fn setup_layout_system(
    mut contexts: EguiContexts,
    mut margins: ResMut<EguiUsedMargins>,
    mut time: ResMut<Time<time::Virtual>>,
    object_query: Query<ObjectTableData>,
    mut write_cameras: WriteCameras,
    mut config_editor_opened: ResMut<config_editor::Opened>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };

    let width = egui::SidePanel::left("level_info")
        .resizable(true)
        .show(ctx, |ui| {
            ui.menu_button("Tools", |ui| {
                if ui.button("Settings").clicked() {
                    config_editor_opened.0 = true;
                    ui.close_menu();
                }
            });

            ui.heading("Level info");

            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new("Time")
                    .default_open(true)
                    .show(ui, |ui| write_time(ui, &mut time));

                ui.collapsing("Camera", |ui| write_cameras.write(ui));

                egui::CollapsingHeader::new("Objects")
                    .default_open(true)
                    .show(ui, |ui| write_objects(ui, &object_query));

                ui.collapsing("Diagnostics", |ui| write_diagnostics(ui, &diagnostics));
            });

            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click());
        })
        .response
        .rect
        .width();
    margins.left += width;
}

fn write_time(ui: &mut egui::Ui, time: &mut ResMut<Time<time::Virtual>>) {
    let elapsed = time.elapsed().as_secs();

    ui.label(format!(
        "Time: {hours}:{minutes:02}:{seconds:02}",
        hours = elapsed / 3600,
        minutes = (elapsed / 60) % 60,
        seconds = elapsed % 60
    ));

    let mut speed = time.relative_speed();
    ui.add(egui::Slider::new(&mut speed, 0. ..=30.).text("Game speed").suffix("x"));
    #[expect(clippy::float_cmp)] // float is exactly equal if nobody touched it
    if speed != time.relative_speed() {
        time.set_relative_speed(speed);
        if speed > 0. {
            time.unpause();
        } else {
            time.pause();
        }
    }
}

#[derive(SystemParam)]
struct WriteCameras<'w, 's> {
    camera_query: Query<
        'w,
        's,
        (&'static Camera, &'static mut Transform, &'static GlobalTransform),
        With<Camera>,
    >,
    cursor:       Res<'w, input::CurrentCursorCamera>,
}

fn measure_delta(v: f32, f: impl FnOnce(&mut f32)) -> Option<f32> {
    let mut copy = v;
    f(&mut copy);
    Some(copy - v).filter(|&v| v != 0.0)
}

impl WriteCameras<'_, '_> {
    fn write(&mut self, ui: &mut egui::Ui) {
        let mut cameras: Vec<_> = self.camera_query.iter_mut().collect();
        cameras.sort_by_key(|(camera, _, _)| match camera.logical_viewport_rect() {
            Some(Rect { min, .. }) => (OrderedFloat(min.x), OrderedFloat(min.y)),
            None => (OrderedFloat::nan(), OrderedFloat::nan()),
        });
        for (i, (camera, mut tf, global_tf)) in cameras.into_iter().enumerate() {
            ui.heading(format!("Camera #{}", i + 1));

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

        if let Some(cursor) = &self.cursor.0 {
            ui.label(format!(
                "Cursor: ({:.1}, {:.1})",
                cursor.world_pos.get().x,
                cursor.world_pos.get().y
            ));
        }
    }
}

#[derive(QueryData)]
struct ObjectTableData {
    display:  &'static object::Display,
    rotation: &'static object::Rotation,
    object:   &'static object::Object,
}

fn write_objects(ui: &mut egui::Ui, object_query: &Query<ObjectTableData>) {
    let columns: Vec<_> = ObjectTableColumn::iter().collect();

    let mut table = TableBuilder::new(ui);
    for _ in 0..columns.len() {
        table = table.column(Column::auto().resizable(true));
    }
    let table = table.header(20., |mut header| {
        for column in &columns {
            header.col(|ui| {
                ui.small(column.header());
            });
        }
    });

    let mut objects: Vec<_> = object_query.iter().collect();
    objects.sort_by_key(|obj| &obj.display.name);

    table.body(|mut body| {
        for object in objects {
            body.row(20., |mut row| {
                for column in &columns {
                    row.col(|ui| column.cell_value(ui, &object));
                }
            });
        }
    });
}

#[derive(strum::EnumIter)]
enum ObjectTableColumn {
    Name,
    Altitude,
    GroundSpeed,
    VerticalRate,
    Heading,
}

impl ObjectTableColumn {
    fn header(&self) -> impl Into<egui::RichText> {
        match self {
            Self::Name => "Name",
            Self::Altitude => "Altitude",
            Self::GroundSpeed => "Ground speed",
            Self::VerticalRate => "Vert rate",
            Self::Heading => "Heading",
        }
    }

    fn cell_value(&self, ui: &mut egui::Ui, data: &ObjectTableDataItem) {
        let text: egui::WidgetText = match self {
            Self::Name => data.display.name.as_str().into(),
            Self::Altitude => {
                format!("{:.0}", &data.object.position.altitude().amsl().into_feet()).into()
            }
            Self::GroundSpeed => format!(
                "{:.0}",
                &data.object.ground_speed.horizontal().magnitude_exact().into_knots()
            )
            .into(),
            Self::VerticalRate => {
                format!("{:+.0}", &data.object.ground_speed.vertical().into_fpm()).into()
            }
            Self::Heading => format!("{:.0}", Heading::from_quat(data.rotation.0).degrees()).into(),
        };
        ui.label(text);
    }
}

fn write_diagnostics(ui: &mut egui::Ui, diagnostics: &DiagnosticsStore) {
    if let Some(fps) =
        diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(Diagnostic::smoothed)
    {
        ui.label(format!("FPS: {fps}"));
    }

    if let Some(entities) =
        diagnostics.get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT).and_then(Diagnostic::value)
    {
        ui.label(format!("Entities: {entities}"));
    }
}

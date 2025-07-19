use std::cmp;

use bevy::app::{App, Plugin};
use bevy::diagnostic::{
    Diagnostic, DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Local, Query, Res, ResMut, SystemParam};
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;
use bevy::math::{Rect, Vec3, Vec3Swizzles};
use bevy::render::camera::Camera;
use bevy::time::{self, Time};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use egui_extras::{Column, TableBuilder};
use math::{Angle, Heading};
use omniatc::level::object;
use ordered_float::{Float, OrderedFloat};
use strum::IntoEnumIterator;

use super::{config_editor, object_info};
use crate::render::twodim;
use crate::util::new_type_id;
use crate::{input, EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            setup_layout_system.in_set(EguiSystemSets::LevelInfo),
        );
    }
}

fn setup_layout_system(
    mut contexts: EguiContexts,
    mut margins: ResMut<EguiUsedMargins>,
    mut write_cameras: WriteCameras,
    mut config_editor_opened: ResMut<config_editor::Opened>,
    diagnostics: Res<DiagnosticsStore>,
    write_time_params: WriteTimeParams,
    write_object_params: WriteObjectParams,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let resp = egui::SidePanel::left(new_type_id!())
        .resizable(true)
        .show(ctx, |ui| {
            if ui.button("Settings").clicked() {
                config_editor_opened.0 = true;
            }

            ui.heading("Level info");

            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new("Time")
                    .default_open(true)
                    .show(ui, |ui| write_time(ui, write_time_params));

                ui.collapsing("Camera", |ui| write_cameras.write(ui));

                egui::CollapsingHeader::new("Objects")
                    .default_open(true)
                    .show(ui, |ui| write_objects(ui, write_object_params));

                ui.collapsing("Diagnostics", |ui| write_diagnostics(ui, &diagnostics));
            });

            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click());
        })
        .response;
    margins.left += resp.rect.width();
}

const FAST_FORWARD_SPEED: f32 = 60.;

#[derive(SystemParam)]
struct WriteTimeParams<'w, 's> {
    time:          ResMut<'w, Time<time::Virtual>>,
    regular_speed: Local<'s, Option<f32>>,
    hotkeys:       Res<'w, input::Hotkeys>,
    paused:        Local<'s, bool>,
}

fn write_time(ui: &mut egui::Ui, mut params: WriteTimeParams) {
    let elapsed = params.time.elapsed().as_secs();

    ui.label(format!(
        "Time: {hours}:{minutes:02}:{seconds:02}",
        hours = elapsed / 3600,
        minutes = (elapsed / 60) % 60,
        seconds = elapsed % 60
    ));

    if params.hotkeys.toggle_pause {
        *params.paused = !*params.paused;
    }

    let desired_speed = ui
        .horizontal(|ui| {
            let regular_speed = params.regular_speed.get_or_insert(1.);
            ui.add(egui::Slider::new(regular_speed, 0. ..=20.).prefix("Game speed: ").suffix("x"));

            if params.hotkeys.fast_forward {
                ui.label(format!("{FAST_FORWARD_SPEED}x"));
                FAST_FORWARD_SPEED
            } else if *params.paused {
                ui.label("Paused");
                0.0
            } else {
                *regular_speed
            }
        })
        .inner;

    #[expect(clippy::float_cmp)] // float is exactly equal if nobody touched it
    if params.time.relative_speed() != desired_speed {
        params.time.set_relative_speed(desired_speed);
        if desired_speed > 0. {
            params.time.unpause();
        } else {
            params.time.pause();
        }
    }
}

#[derive(SystemParam)]
struct WriteCameras<'w, 's> {
    camera_query: Query<
        'w,
        's,
        (&'static Camera, &'static mut Transform, &'static GlobalTransform),
        With<twodim::camera::Layout>,
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
    entity:   Entity,
    display:  &'static object::Display,
    rotation: &'static object::Rotation,
    object:   &'static object::Object,
}

#[derive(SystemParam)]
struct WriteObjectParams<'w, 's> {
    object_query:    Query<'w, 's, ObjectTableData>,
    last_rows:       Local<'s, Option<usize>>,
    sort_key:        Local<'s, (usize, bool)>,
    search_str:      Local<'s, String>,
    keys:            Res<'w, ButtonInput<KeyCode>>,
    hotkeys:         Res<'w, input::Hotkeys>,
    selected_object: ResMut<'w, object_info::CurrentObject>,
}

fn write_objects(ui: &mut egui::Ui, mut params: WriteObjectParams) {
    let mut select_first = false;
    ui.horizontal(|ui| {
        ui.label("Search");

        let search_resp = ui.add(
            egui::TextEdit::singleline(&mut *params.search_str)
                .hint_text("'/' to focus, Enter to select first match"),
        );
        if search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            select_first = true;
        }
        if search_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Slash)) {
            params.search_str.clear();
        }
        if search_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            search_resp.surrender_focus();
        }
        if params.hotkeys.search {
            search_resp.request_focus();
        }
    });

    if params.hotkeys.deselect {
        params.selected_object.0 = None;
    }

    let columns: Vec<_> = ObjectTableColumn::iter().collect();
    let mut objects: Vec<_> = params
        .object_query
        .iter()
        .filter(|object| {
            if params.search_str.is_empty() {
                true
            } else {
                object.display.name.to_lowercase().contains(&params.search_str.to_lowercase())
            }
        })
        .collect();
    let rows_changed = params.last_rows.replace(objects.len()) != Some(objects.len());

    TableBuilder::new(ui)
        .columns(Column::auto().resizable(true).auto_size_this_frame(rows_changed), columns.len())
        .header(20., |mut header| {
            for (column_id, column) in columns.iter().enumerate() {
                header.col(|ui| {
                    let mut clicked = false;
                    ui.horizontal(|ui| {
                        clicked |= ui.small(column.header()).clicked();
                        if params.sort_key.0 == column_id {
                            clicked |=
                                ui.label(if params.sort_key.1 { "v" } else { "^" }).clicked();
                        }
                    });
                    if clicked {
                        if params.sort_key.0 == column_id {
                            params.sort_key.1 = !params.sort_key.1;
                        } else {
                            *params.sort_key = (column_id, false);
                        }
                    }
                });
            }
        })
        .body(|body| {
            columns[params.sort_key.0].sort(&mut objects[..], params.sort_key.1);

            if select_first {
                if let Some(first) = objects.first() {
                    params.selected_object.0 = Some(first.entity);
                }
            }

            body.rows(20., objects.len(), |mut row| {
                let object = &objects[row.index()];
                let is_selected = params.selected_object.0 == Some(object.entity);
                row.set_selected(is_selected);

                for column in &columns {
                    row.col(|ui| column.cell_value(ui, object));
                }
            });
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
            Self::Name => "Object name",
            Self::Altitude => "Altitude (ft)",
            Self::GroundSpeed => "Ground\nspeed (kt)",
            Self::VerticalRate => "Vert rate\n(fpm)",
            Self::Heading => "Heading",
        }
    }

    fn cell_value(&self, ui: &mut egui::Ui, data: &ObjectTableDataItem) {
        let text: egui::WidgetText = match self {
            Self::Name => egui::WidgetText::from(data.display.name.as_str()).strong(),
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
            Self::Heading => {
                format!("{:.0}\u{b0}", Heading::from_quat(data.rotation.0).degrees()).into()
            }
        };
        ui.label(text);
    }

    fn sort(&self, objects: &mut [ObjectTableDataItem], desc: bool) {
        match self {
            Self::Name => objects.sort_by_key(|data| ConditionalReverse(desc, &data.display.name)),
            Self::Altitude => objects.sort_by_key(|data| {
                ConditionalReverse(desc, OrderedFloat(data.object.position.altitude().get()))
            }),
            Self::GroundSpeed => objects.sort_by_key(|data| {
                ConditionalReverse(desc, data.object.ground_speed.horizontal().magnitude_cmp())
            }),
            Self::VerticalRate => objects.sort_by_key(|data| {
                ConditionalReverse(desc, OrderedFloat(data.object.ground_speed.vertical().0))
            }),
            Self::Heading => objects.sort_by_key(|data| {
                ConditionalReverse(
                    desc,
                    OrderedFloat(Heading::from_quat(data.rotation.0).radians().0),
                )
            }),
        }
    }
}

#[derive(PartialEq, Eq)]
struct ConditionalReverse<T>(bool, T);

impl<T: Ord> PartialOrd for ConditionalReverse<T> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(self.cmp(other)) }
}

impl<T: Ord> Ord for ConditionalReverse<T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.0 {
            other.1.cmp(&self.1)
        } else {
            self.1.cmp(&other.1)
        }
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

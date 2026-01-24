use std::cmp;

use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Local, Query, Res, ResMut, SystemParam};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy_egui::egui;
use egui_extras::{Column, TableBuilder};
use math::Heading;
use omniatc::level::object;
use ordered_float::OrderedFloat;
use strum::IntoEnumIterator;

use super::WriteParams;
use crate::input;
use crate::render::object_info;

#[derive(QueryData)]
pub struct ObjectTableData {
    entity:   Entity,
    display:  &'static object::Display,
    rotation: &'static object::Rotation,
    object:   &'static object::Object,
}

#[derive(SystemParam)]
pub struct WriteObjectParams<'w, 's> {
    object_query:    Query<'w, 's, ObjectTableData>,
    last_rows:       Local<'s, Option<usize>>,
    sort_key:        Local<'s, (usize, bool)>,
    search_str:      Local<'s, String>,
    keys:            Res<'w, ButtonInput<KeyCode>>,
    hotkeys:         Res<'w, input::Hotkeys>,
    selected_object: ResMut<'w, object_info::CurrentObject>,
}

impl WriteParams for WriteObjectParams<'_, '_> {
    fn title(&self) -> String { format!("Objects ({})", self.object_query.iter().len()) }

    fn default_open() -> bool { true }

    fn write(&mut self, ui: &mut egui::Ui) {
        let mut select_first = false;
        ui.horizontal(|ui| {
            ui.label("Search");

            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut *self.search_str)
                    .hint_text("'/' to focus, Enter to select first match"),
            );
            if search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                select_first = true;
            }
            if search_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Slash)) {
                self.search_str.clear();
            }
            if search_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                search_resp.surrender_focus();
            }
            if self.hotkeys.search {
                search_resp.request_focus();
            }
        });

        if self.hotkeys.deselect {
            self.selected_object.0 = None;
        }

        let columns: Vec<_> = ObjectTableColumn::iter().collect();
        let mut objects: Vec<_> = self
            .object_query
            .iter()
            .filter(|object| {
                if self.search_str.is_empty() {
                    true
                } else {
                    object.display.name.to_lowercase().contains(&self.search_str.to_lowercase())
                }
            })
            .collect();
        let rows_changed = self.last_rows.replace(objects.len()) != Some(objects.len());

        TableBuilder::new(ui)
            .columns(
                Column::auto().resizable(true).auto_size_this_frame(rows_changed),
                columns.len(),
            )
            .header(20., |mut header| {
                for (column_id, column) in columns.iter().enumerate() {
                    header.col(|ui| {
                        let mut clicked = false;
                        ui.horizontal(|ui| {
                            clicked |= ui.small(column.header()).clicked();
                            if self.sort_key.0 == column_id {
                                clicked |=
                                    ui.label(if self.sort_key.1 { "v" } else { "^" }).clicked();
                            }
                        });
                        if clicked {
                            if self.sort_key.0 == column_id {
                                self.sort_key.1 = !self.sort_key.1;
                            } else {
                                *self.sort_key = (column_id, false);
                            }
                        }
                    });
                }
            })
            .body(|body| {
                columns[self.sort_key.0].sort(&mut objects[..], self.sort_key.1);

                if select_first && let Some(first) = objects.first() {
                    self.selected_object.0 = Some(first.entity);
                }

                body.rows(20., objects.len(), |mut row| {
                    let object = &objects[row.index()];
                    let is_selected = self.selected_object.0 == Some(object.entity);
                    row.set_selected(is_selected);

                    let mut clicked = false;
                    for column in &columns {
                        row.col(|ui| {
                            let resp = column.cell_value(ui, object);
                            clicked |= resp.clicked();
                        });
                    }
                    if clicked {
                        self.selected_object.0 = Some(object.entity);
                    }
                });
            });
    }
}

#[derive(strum::EnumIter)]
enum ObjectTableColumn {
    Name,
    Altitude,
    GroundSpeed,
    VerticalRate,
    Heading,
}

#[derive(PartialEq, Eq)]
struct ConditionalReverse<T>(bool, T);

impl<T: Ord> PartialOrd for ConditionalReverse<T> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(self.cmp(other)) }
}

impl<T: Ord> Ord for ConditionalReverse<T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.0 { other.1.cmp(&self.1) } else { self.1.cmp(&other.1) }
    }
}

impl ObjectTableColumn {
    fn header(&self) -> impl Into<egui::RichText> {
        match self {
            Self::Name => "Callsign",
            Self::Altitude => "Altitude (ft)",
            Self::GroundSpeed => "Ground\nspeed (kt)",
            Self::VerticalRate => "Vert rate\n(fpm)",
            Self::Heading => "Heading",
        }
    }

    fn cell_value(&self, ui: &mut egui::Ui, data: &ObjectTableDataItem) -> egui::Response {
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
        ui.label(text)
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

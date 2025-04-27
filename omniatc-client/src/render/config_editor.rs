use std::any::TypeId;

use bevy::app::{App, Plugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Res, ResMut};
use bevy_egui::{egui, EguiContextPass, EguiContexts};

use crate::config::Config;
use crate::{config, EguiSystemSets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Opened>();
        app.add_systems(EguiContextPass, setup_messages_system.in_set(EguiSystemSets::Config));
    }
}

#[derive(Default, Resource)]
pub struct Opened(pub bool);

fn setup_messages_system(
    mut contexts: EguiContexts,
    mut opened: ResMut<Opened>,
    registry: Res<config::Registry>,
    mut configs: ResMut<config::Values>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    if !opened.0 {
        return;
    }

    let default_size = ctx.screen_rect().size() / 2.;
    egui::Window::new("Settings")
        .default_size(default_size)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200),
            ..Default::default()
        })
        .show(ctx, |ui| {
            if ui.button("Close").clicked() {
                opened.0 = false;
                return;
            }

            ui.horizontal(|ui| {
                struct Left;
                struct Right;

                ui.set_min_height(default_size.y);
                ui.set_min_width(default_size.x);

                let mut doc = String::new();

                egui::ScrollArea::vertical().id_salt(TypeId::of::<Left>()).show(ui, |ui| {
                    ui.vertical(|ui| {
                        for (i, ty) in registry.0.iter().enumerate() {
                            egui::CollapsingHeader::new(ty.name).default_open(i == 0).show(
                                ui,
                                |ui| {
                                    (ty.draw)(&mut configs, ui, &mut doc);
                                },
                            );
                        }
                    });
                });

                egui::ScrollArea::vertical().id_salt(TypeId::of::<Right>()).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.add(egui::Label::new(doc));
                    });
                })
            });
        });
}

pub fn draw<C: Config>(values: &mut config::Values, ui: &mut egui::Ui, doc: &mut String) {
    struct DrawVisitor<'a> {
        ui: &'a mut egui::Ui,
    }

    impl config::FieldVisitor for DrawVisitor<'_> {
        fn visit_field<F: config::Field>(
            &mut self,
            meta: config::FieldMeta<F::Opts>,
            field: &mut F,
            ctx: &mut config::FieldEguiContext,
        ) {
            field.show_egui(meta, self.ui, ctx);
        }
    }

    let store = values.0.get_mut(&TypeId::of::<C>()).expect("registered type must exist in Values");
    let value = store.value.downcast_mut::<C>().expect("TypeId mismatch");

    let mut visitor = DrawVisitor { ui };
    let mut changed = false;
    let mut ctx = config::FieldEguiContext { changed: &mut changed, doc };
    value.for_each_field(&mut visitor, &mut ctx);

    if changed {
        store.generation = store.generation.wrapping_add(1);
    }
}

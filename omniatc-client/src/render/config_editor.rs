use std::any::TypeId;

use bevy::app::{App, Plugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Res, ResMut};
use bevy_egui::{egui, EguiContextPass, EguiContexts};

use crate::config::Config;
use crate::{config, EguiSystemSets};

const DEFAULT_WINDOW_SIZE: egui::Vec2 = egui::Vec2::new(300., 200.);

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

    egui::Window::new("Settings").default_size(DEFAULT_WINDOW_SIZE).show(ctx, |ui| {
        if ui.button("Close").clicked() {
            opened.0 = false;
            return;
        }

        for ty in &registry.0 {
            ui.heading(ty.name);
            (ty.draw)(&mut configs, ui);
        }
    });
}

pub fn draw<C: Config>(values: &mut config::Values, ui: &mut egui::Ui) {
    struct DrawVisitor<'a> {
        ui:      &'a mut egui::Ui,
        changed: bool,
    }

    impl config::FieldVisitor for DrawVisitor<'_> {
        fn visit_field<F: config::Field>(
            &mut self,
            meta: config::FieldMeta<F::Opts>,
            field: &mut F,
        ) {
            field.show_egui(meta, self.ui, &mut self.changed);
        }
    }

    let store = values.0.get_mut(&TypeId::of::<C>()).expect("registered type must exist in Values");
    let value = store.value.downcast_mut::<C>().expect("TypeId mismatch");

    let mut visitor = DrawVisitor { ui, changed: false };
    value.for_each_field(&mut visitor);

    if visitor.changed {
        store.generation = store.generation.wrapping_add(1);
    }
}

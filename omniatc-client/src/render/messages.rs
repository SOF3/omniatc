use std::collections::VecDeque;
use std::time::Instant;

use bevy::app::{App, Plugin};
use bevy::color::Color;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;
use bevy_egui::{egui, EguiContextPass, EguiContexts};

use crate::{EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Messages>();
        app.add_systems(EguiContextPass, setup_messages_system.in_set(EguiSystemSets::Messages));
    }
}

#[derive(Default, Resource)]
pub struct Messages(pub VecDeque<Message>);

pub struct Message {
    pub expiry:  Instant,
    pub content: String,
    pub color:   Color,
}

fn setup_messages_system(
    mut contexts: EguiContexts,
    mut messages: ResMut<Messages>,
    mut margins: ResMut<EguiUsedMargins>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };

    let height = egui::TopBottomPanel::bottom("messages")
        .resizable(true)
        .show(ctx, |ui| {
            ui.label("Messages");
            egui::ScrollArea::vertical().show(ui, |ui| {
                while let Some(message) = messages.0.front() {
                    if message.expiry < Instant::now() {
                        messages.0.pop_front();
                    } else {
                        break;
                    }
                }

                for message in &messages.0 {
                    let color = message.color.to_srgba();

                    // rgba should be within [0., 1.]
                    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                    let color = egui::Rgba::from_srgba_premultiplied(
                        (color.red * 255.) as u8,
                        (color.green * 255.) as u8,
                        (color.blue * 255.) as u8,
                        (color.alpha * 255.) as u8,
                    );
                    ui.add(
                        egui::Label::new(egui::RichText::new(&message.content).color(color))
                            .selectable(true),
                    );
                }
            });
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
        })
        .response
        .rect
        .height();
    margins.bottom += height;
}

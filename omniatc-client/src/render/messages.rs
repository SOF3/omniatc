use std::collections::VecDeque;
use std::time::{Duration, Instant};

use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::ecs::event::EventReader;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Command, Commands, Query, ResMut};
use bevy::ecs::world::World;
use bevy_egui::{egui, EguiContextPass, EguiContexts};
use omniatc::level::message;
use omniatc::try_log;

use crate::{EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Messages>();
        app.add_systems(EguiContextPass, setup_messages_system.in_set(EguiSystemSets::Messages));
        app.add_systems(app::Update, sync_message_system.ambiguous_with_all());
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

pub struct SendMessage {
    pub content:  String,
    pub color:    Color,
    pub duration: Duration,
}

impl Command for SendMessage {
    fn apply(self, world: &mut World) {
        let mut messages = world.resource_mut::<Messages>();
        messages.0.push_back(Message {
            expiry:  Instant::now() + self.duration,
            content: self.content,
            color:   self.color,
        });
    }
}

fn sync_message_system(
    mut events: EventReader<message::SendEvent>,
    sender_query: Query<&message::Sender>,
    mut commands: Commands,
) {
    for event in events.read() {
        let message::Sender { display } = try_log!(sender_query.get(event.source), expect "message source must be a message::Sender" or continue);
        commands.queue(SendMessage {
            content:  format!("{display}: {}", &event.message),
            color:    match event.class {
                message::Class::Urgent => Color::srgb(1., 0.6, 0.8),
                message::Class::AnomalyInfo => Color::srgb(0.9, 1., 0.6),
                message::Class::NeedAck => Color::srgb(0.6, 0.8, 1.),
                message::Class::VerboseInfo => Color::srgb(0.5, 0.7, 0.5),
            },
            duration: Duration::from_secs(60),
            // TODO rewrite this plugin, messages should be entities instead of always auto expire
        });
    }
}

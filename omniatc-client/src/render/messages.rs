use bevy::app::{App, Plugin};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, SystemParam};
use bevy::time::{self, Time};
use bevy_egui::egui;
use bevy_egui::egui::text::LayoutJob;
use egui_dock::DockState;
use omniatc::level::message::{self, Message};

use crate::render::dock;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, _app: &mut App) {}
}

fn color_from_class(class: message::Class) -> Color {
    match class {
        message::Class::Outgoing => Color::srgb(0.6, 0.6, 0.8),
        message::Class::Urgent => Color::srgb(1., 0.6, 0.8),
        message::Class::AnomalyInfo => Color::srgb(0.9, 1., 0.6),
        message::Class::NeedAck => Color::srgb(0.6, 0.8, 1.),
        message::Class::VerboseInfo => Color::srgb(0.5, 0.7, 0.5),
    }
}

pub struct TabType;

#[derive(SystemParam)]
pub struct UiParams<'w, 's> {
    messages: Query<'w, 's, (Entity, &'static Message)>,
    senders:  Query<'w, 's, &'static message::Sender>,
    time:     Res<'w, Time<time::Virtual>>,
    commands: Commands<'w, 's>,
}

impl dock::TabType for TabType {
    type TitleSystemParam<'w, 's> = Query<'w, 's, (Entity, &'static Message)>;
    fn title(&self, param: Self::TitleSystemParam<'_, '_>) -> String {
        format!("Messages ({})", param.iter().len())
    }

    type UiSystemParam<'w, 's> = UiParams<'w, 's>;
    fn ui(&mut self, mut params: Self::UiSystemParam<'_, '_>, ui: &mut egui::Ui, _order: usize) {
        let mut messages: Vec<_> = params.messages.into_iter().collect();
        messages.sort_by_key(|(_, message)| message.created);

        for (entity, message) in messages {
            let mut job = LayoutJob::default();

            let sender = match params.senders.get(message.source) {
                Ok(sender) => format!("{}: ", &sender.display),
                _ => continue, // format!("<sender {:?}>: ", message.source),
            };
            job.append(
                &sender,
                0.,
                egui::TextFormat {
                    color: egui::Color32::from_rgba_unmultiplied(180, 180, 180, 255),
                    ..Default::default()
                },
            );

            let color = color_from_class(message.class).to_srgba();
            #[expect(
                clippy::cast_sign_loss,
                clippy::cast_possible_truncation,
                reason = "rgba should be within [0., 1.]"
            )]
            let color = egui::Color32::from_rgba_unmultiplied(
                (color.red * 255.) as u8,
                (color.green * 255.) as u8,
                (color.blue * 255.) as u8,
                (color.alpha * 255.) as u8,
            );

            job.append(&message.content, 0., egui::TextFormat { color, ..Default::default() });

            #[expect(
                clippy::unchecked_time_subtraction,
                reason = "time.elapsed() is monotonic increasing"
            )]
            job.append(
                &format!(" [{:.1}s]", (params.time.elapsed() - message.created).as_secs_f32()),
                0.,
                egui::TextFormat {
                    color: egui::Color32::from_rgba_unmultiplied(150, 150, 150, 255),
                    ..Default::default()
                },
            );

            let resp = ui.add(egui::Label::new(job).wrap());
            if resp.clicked() {
                params.commands.entity(entity).despawn();
            }
        }
    }

    type OnCloseSystemParam<'w, 's> = ();

    type PrepareRenderSystemParam<'w, 's> = ();
}

pub(super) fn create_splits(dock: &mut DockState<dock::Tab>) {
    dock.split(
        (egui_dock::SurfaceIndex::main(), egui_dock::NodeIndex::root()),
        egui_dock::Split::Below,
        0.8,
        egui_dock::Node::leaf(dock::Tab::Messages(TabType)),
    );
}

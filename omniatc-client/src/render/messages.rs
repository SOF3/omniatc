use bevy::app::{App, Plugin};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::time::{self, Time};
use bevy_egui::egui::text::LayoutJob;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use omniatc::level::message::{self, Message};

use crate::util::new_type_id;
use crate::{EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            setup_messages_system.in_set(EguiSystemSets::Messages),
        );
    }
}

fn setup_messages_system(
    mut contexts: EguiContexts,
    mut margins: ResMut<EguiUsedMargins>,
    messages: Query<(Entity, &Message)>,
    senders: Query<&message::Sender>,
    time: Res<Time<time::Virtual>>,
    mut commands: Commands,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let height = egui::TopBottomPanel::bottom(new_type_id!())
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Messages");
            egui::ScrollArea::vertical().id_salt(new_type_id!()).show(ui, |ui| {
                let mut messages: Vec<_> = messages.into_iter().collect();
                messages.sort_by_key(|(_, message)| message.created);

                for (entity, message) in messages {
                    let mut job = LayoutJob::default();

                    let sender = match senders.get(message.source) {
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
                    // rgba should be within [0., 1.]
                    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                    let color = egui::Color32::from_rgba_unmultiplied(
                        (color.red * 255.) as u8,
                        (color.green * 255.) as u8,
                        (color.blue * 255.) as u8,
                        (color.alpha * 255.) as u8,
                    );

                    job.append(
                        &message.content,
                        0.,
                        egui::TextFormat { color, ..Default::default() },
                    );

                    #[expect(
                        clippy::unchecked_time_subtraction,
                        reason = "time.elapsed() is monotonic increasing"
                    )]
                    job.append(
                        &format!(" [{:.1}s]", (time.elapsed() - message.created).as_secs_f32()),
                        0.,
                        egui::TextFormat {
                            color: egui::Color32::from_rgba_unmultiplied(150, 150, 150, 255),
                            ..Default::default()
                        },
                    );

                    let resp = ui.add(egui::Label::new(job).wrap());
                    if resp.clicked() {
                        commands.entity(entity).despawn();
                    }
                }
            });
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
        })
        .response
        .rect
        .height();
    margins.bottom += height;
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

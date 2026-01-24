use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::EguiSystemSets;
use crate::render::level_info;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            setup_window_system.in_set(EguiSystemSets::Tutorial),
        );
    }
}

#[derive(Component)]
pub struct Focused;

fn setup_window_system(
    mut contexts: EguiContexts,
    quest_query: Query<(level_info::quests::QuestData, Has<Focused>)>,
    focused_query: Query<Entity, With<Focused>>,
    mut commands: Commands,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let Some((quest, focused)) = quest_query
        .into_iter()
        .filter(|(quest, _)| quest.quest.class.display_in_popup() && quest.active)
        .min_by_key(|(quest, _)| quest.quest.index)
    else {
        return;
    };
    if !focused {
        commands.entity(quest.entity).insert(Focused);
    }

    for other in focused_query {
        if other != quest.entity {
            commands.entity(other).remove::<Focused>();
        }
    }

    let default_size = ctx.content_rect().size() / 2.;
    egui::Window::new("Tutorial")
        .default_size(default_size)
        .default_open(true)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200),
            ..Default::default()
        })
        .show(ctx, |ui| {
            quest.show(ui, &mut commands);
        });
}

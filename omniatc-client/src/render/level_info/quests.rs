use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, QueryData};
use bevy::ecs::system::{Commands, Query, SystemParam};
use bevy_egui::egui;
use omniatc::level::quest::{self, Quest};

#[derive(SystemParam)]
pub struct WriteQuestsParams<'w, 's> {
    quest_query: Query<'w, 's, QuestData>,
    commands:    Commands<'w, 's>,
}

#[derive(QueryData)]
pub struct QuestData {
    pub entity: Entity,
    pub quest:  &'static Quest,
    pub active: Has<quest::Active>,
}

impl WriteQuestsParams<'_, '_> {
    pub fn display(&mut self, ui: &mut egui::Ui) {
        let mut quests: Vec<_> = self
            .quest_query
            .iter()
            .filter(|d| d.active && d.quest.class.display_in_list())
            .collect();
        quests.sort_by_key(|d| d.quest.index);
        for d in quests {
            egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
                d.show(ui, &mut self.commands);
            });
        }
    }
}

impl QuestDataItem<'_, '_> {
    pub fn show(&self, ui: &mut egui::Ui, commands: &mut Commands) {
        ui.horizontal(|ui| {
            ui.heading(self.quest.title.clone());

            if self.quest.class.is_skippable() {
                let resp = ui.small_button("Skip");
                if resp.clicked() {
                    commands.entity(self.entity).remove::<quest::condition::AllBundle>();
                }
            }
        });
        ui.label(self.quest.description.clone());
    }
}

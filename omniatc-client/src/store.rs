use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;

use bevy::app::{self, App, Plugin};
use bevy::asset::AssetApp;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, ResMut};
use omniatc_core::store;
use serde::{Deserialize, Serialize};

mod scenario_loader;

#[cfg(target_family = "wasm")]
mod web;
#[cfg(target_family = "wasm")]
pub type StorageImpl = web::Impl;

#[cfg(not(target_family = "wasm"))]
mod fs;
#[cfg(not(target_family = "wasm"))]
pub type StorageImpl = fs::Impl;

#[derive(Serialize, Deserialize)]
pub struct ScenarioMeta {
    key:     String,
    title:   String,
    created: SystemTime,
}

#[derive(Serialize, Deserialize)]
pub struct LevelMeta {
    key:      String,
    title:    String,
    created:  SystemTime,
    modified: SystemTime,
}

pub trait Storage: Resource {
    type Error: fmt::Debug;

    fn list_scenarios_by_tag(&mut self, tag_key: &str) -> anyhow::Result<Vec<ScenarioMeta>>;
    fn load_scenario(&mut self, key: &str) -> Result<Vec<u8>, Self::Error>;
    fn insert_scenario(
        &mut self,
        meta: ScenarioMeta,
        data: &[u8],
        tags: &HashMap<String, String>,
    ) -> Result<(), Self::Error>;

    fn list_levels_by_time(&mut self, limit: usize) -> anyhow::Result<Vec<LevelMeta>>;
    fn load_level(&mut self, key: &str) -> Result<Vec<u8>, Self::Error>;
}

pub struct Plug<S>(pub fn() -> S);

pub fn plugin() -> Plug<impl Storage> {
    Plug(|| StorageImpl::try_new().expect("storage error")) // TODO handle error properly
}

impl<S: Storage> Plugin for Plug<S> {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0());
        app.init_resource::<scenario_loader::CurrentImportingScenarios>();
        app.init_resource::<scenario_loader::CurrentLoadOnImport>();
        app.init_asset::<scenario_loader::ScenarioAsset>();
        app.init_asset_loader::<scenario_loader::ScenarioAssetLoader>();
        app.add_systems(app::Startup, scenario_loader::import_builtin_scenarios_system);
        app.add_systems(app::Startup, load_last_level_system::<S>);
        app.add_systems(app::Update, scenario_loader::handle_loaded_scenario_system::<S>);
    }
}

fn load_last_level_system<S: Storage>(
    mut storage: ResMut<S>,
    mut commands: Commands,
    mut current_load_on_import: ResMut<scenario_loader::CurrentLoadOnImport>,
) {
    let key = match storage.list_levels_by_time(1) {
        Ok(list) => list.into_iter().next().map(|level| level.key),
        Err(err) => {
            bevy::log::error!("Cannot locate last available level: {err:?}");
            None
        }
    };

    let source = key.and_then(|key| match storage.load_level(&key) {
        Ok(data) => Some(store::load::Source::Raw(Cow::Owned(data))),
        Err(err) => {
            bevy::log::error!("Cannot load last loaded level: {err:?}");
            None
        }
    });

    if let Some(source) = source {
        commands.queue(store::load::Command {
            source,
            on_error: Box::new(|_world, err| bevy::log::error!("Error loading level: {err:?}")),
        });
    } else {
        current_load_on_import.0 = Some(scenario_loader::DEFAULT_SCENARIO.into());
    }
}

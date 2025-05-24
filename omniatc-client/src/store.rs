use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::time::SystemTime;

use bevy::app::{self, App, Plugin};
use bevy::asset::AssetApp;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, NonSend, Res, ResMut};
use omniatc::store;
use omniatc::util::{run_async_local, AsyncPollList, AsyncResult};
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
    id:      String,
    title:   String,
    created: SystemTime,
}

#[derive(Serialize, Deserialize)]
pub struct LevelMeta {
    id:       String,
    title:    String,
    created:  SystemTime,
    modified: SystemTime,
}

pub trait Storage: Default + 'static {
    type Error: fmt::Debug + Send + Sync + 'static;

    fn list_scenarios_by_tag(
        &self,
        tag_key: String,
    ) -> impl Future<Output = anyhow::Result<Vec<ScenarioMeta>>> + 'static;
    fn load_scenario(
        &self,
        key: String,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + 'static;
    fn insert_scenario(
        &self,
        meta: ScenarioMeta,
        data: Vec<u8>,
        tags: HashMap<String, String>,
    ) -> impl Future<Output = Result<(), Self::Error>> + 'static;

    fn list_levels_by_time(
        &self,
        limit: usize,
    ) -> impl Future<Output = anyhow::Result<Vec<LevelMeta>>> + 'static;
    fn load_level(
        &self,
        key: String,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + 'static;
}

pub struct Plug<S>(pub fn() -> S);

pub fn plugin() -> Plug<impl Storage> { Plug(StorageImpl::default) }

impl<S: Storage> Plugin for Plug<S> {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(self.0());
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
    mut commands: Commands,
    mut poll_list: ResMut<AsyncPollList>,
    storage: NonSend<S>,
) {
    run_async_local(storage.list_levels_by_time(1)).then(
        &mut commands,
        &mut poll_list,
        |mut ret: AsyncResult<anyhow::Result<Vec<LevelMeta>>>,
         storage: NonSend<S>,
         mut current_load_on_import: ResMut<scenario_loader::CurrentLoadOnImport>,
         mut poll_list: ResMut<AsyncPollList>,
         mut commands: Commands| {
            let key = match ret.get() {
                Ok(list) => list.into_iter().next().map(|level| level.id),
                Err(err) => {
                    bevy::log::error!("Cannot locate last available level: {err:?}");
                    None
                }
            };

            match key {
                Some(key) => {
                    run_async_local(storage.load_level(key)).then(
                        &mut commands,
                        &mut poll_list,
                        |mut ret: AsyncResult<Result<Vec<u8>, S::Error>>,
                         mut commands: Commands,
                         mut current_load_on_import: ResMut<
                            scenario_loader::CurrentLoadOnImport,
                        >| match ret.get() {
                            Ok(data) => {
                                let source = store::load::Source::Raw(Cow::Owned(data));
                                commands.queue(store::load::Command {
                                    source,
                                    on_error: Box::new(|_world, err| {
                                        bevy::log::error!("Error loading level: {err:?}");
                                    }),
                                });
                            }
                            Err(err) => {
                                bevy::log::error!("Cannot load last loaded level: {err:?}");
                                current_load_on_import.0 =
                                    Some(scenario_loader::DEFAULT_SCENARIO.into());
                            }
                        },
                    );
                }
                None => current_load_on_import.0 = Some(scenario_loader::DEFAULT_SCENARIO.into()),
            }
        },
    );
}

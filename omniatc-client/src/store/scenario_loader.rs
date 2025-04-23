use std::{io, mem};

use bevy::asset::{
    self, Asset, AssetLoader, AssetPath, AssetServer, Assets, DirectAssetAccessExt, Handle,
};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Command, Commands, Res, ResMut};
use bevy::ecs::world::World;
use bevy::reflect::TypePath;
use bevy::tasks::ConditionalSendFuture;
use omniatc_core::store;

use super::{ScenarioMeta, Storage};
use crate::util;

#[derive(Asset, TypePath)]
pub struct ScenarioAsset {
    pub bytes: Vec<u8>,
    file:      store::File,
}

#[derive(Default)]
pub struct ScenarioAssetLoader;

impl AssetLoader for ScenarioAssetLoader {
    type Asset = ScenarioAsset;
    type Settings = ();
    type Error = ciborium::de::Error<io::Error>;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        (): &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>> {
        async {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            ciborium::from_reader(&bytes[..]).map(|file| ScenarioAsset { bytes, file })
        }
    }
}

pub struct ImportScenario<P>(pub P);

impl<P: Into<AssetPath<'static>> + Send + 'static> Command for ImportScenario<P> {
    fn apply(self, world: &mut World) {
        let handle = world.load_asset(self.0);
        world.resource_mut::<CurrentImportingScenarios>().0.push(handle);
    }
}

#[derive(Default, Resource)]
pub struct CurrentImportingScenarios(Vec<Handle<ScenarioAsset>>);

/// Load the scenario into the active level when import completes.
#[derive(Default, Resource)]
pub struct CurrentLoadOnImport(pub Option<String>);

pub fn handle_loaded_scenario_system<S: Storage>(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<Assets<ScenarioAsset>>,
    mut current_importing: ResMut<CurrentImportingScenarios>,
    mut current_load_on_import: ResMut<CurrentLoadOnImport>,
    mut storage: ResMut<S>,
) {
    let mut still_importing = Vec::new();
    for handle in mem::take(&mut current_importing.0) {
        match asset_server.get_load_state(&handle) {
            Some(asset::LoadState::Loaded) => {}
            Some(asset::LoadState::Failed(err)) => {
                bevy::log::error!("Asset load error: {err}");
                continue;
            }
            _ => {
                still_importing.push(handle);
                continue;
            }
        }

        let ScenarioAsset { bytes, file } =
            assets.remove(&handle).expect("asset load state is Loaded");

        if let Err(err) = storage.insert_scenario(
            ScenarioMeta {
                key:     file.meta.id.clone(),
                title:   file.meta.title.clone(),
                created: util::time_now(),
            },
            &bytes,
            &file.meta.tags,
        ) {
            bevy::log::error!("storing imported scenario: {err:?}");
        }

        if current_load_on_import.0.as_ref() == Some(&file.meta.id) {
            current_load_on_import.0 = None;
            commands.queue(store::load::Command {
                source:   store::load::Source::Parsed(Box::new(file)),
                on_error: Box::new(|_world, err| bevy::log::error!("Load error: {err}")),
            });
        }
    }
    current_importing.0 = still_importing;
}

pub const BUILTIN_SCENARIOS: &[&str] = &["maps/example.osav"];
pub const DEFAULT_SCENARIO: &str = "omniatc.example";

pub fn import_builtin_scenarios_system(mut commands: Commands) {
    for &scenario in BUILTIN_SCENARIOS {
        commands.queue(ImportScenario(scenario));
    }
}

use std::mem;

use bevy::asset::{
    self, Asset, AssetLoader, AssetPath, AssetServer, Assets, DirectAssetAccessExt, Handle,
};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Command, Commands, NonSend, Res, ResMut};
use bevy::ecs::world::World;
use bevy::reflect::TypePath;
use bevy::tasks::ConditionalSendFuture;
use jiff::Timestamp;
use omniatc::load;
use omniatc::util::{AsyncManager, AsyncResult, run_async_local};

use super::{ScenarioMeta, Storage};

#[derive(Asset, TypePath)]
pub struct ScenarioAsset {
    pub bytes: Vec<u8>,
    file:      store::File,
}

#[derive(Default, TypePath)]
pub struct ScenarioAssetLoader;

impl AssetLoader for ScenarioAssetLoader {
    type Asset = ScenarioAsset;
    type Settings = ();
    type Error = store::FileDeError;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        (): &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>> {
        async {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await.map_err(store::FileDeError::Io)?;
            store::File::from_osav(&bytes[..]).map(|file| ScenarioAsset { bytes, file })
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

pub(super) fn handle_loaded_scenario_system<S: Storage>(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<Assets<ScenarioAsset>>,
    mut current_importing: ResMut<CurrentImportingScenarios>,
    storage: NonSend<S>,
    mut poll_list: ResMut<AsyncManager>,
) {
    let prev_importing = mem::take(&mut current_importing.0);
    for handle in prev_importing {
        match asset_server.get_load_state(&handle) {
            Some(asset::LoadState::Loaded) => {}
            Some(asset::LoadState::Failed(err)) => {
                bevy::log::error!("Asset load error: {err}");
                continue;
            }
            _ => {
                current_importing.0.push(handle);
                continue;
            }
        }

        let ScenarioAsset { bytes, file } =
            assets.remove(&handle).expect("asset load state is Loaded");

        run_async_local(storage.insert_scenario(
            ScenarioMeta {
                id:      file.meta.id.clone(),
                title:   file.meta.title.clone(),
                created: Timestamp::now(),
            },
            bytes,
            file.meta.tags.clone(),
        ))
        .then(&mut commands, &mut poll_list, {
            let mut file = Some(file);
            move |mut ret: AsyncResult<Result<(), S::Error>>,
                  mut commands: Commands,
                  mut current_load_on_import: ResMut<CurrentLoadOnImport>| {
                if let Err(err) = ret.get() {
                    bevy::log::error!("storing imported scenario: {err:?}");
                    return;
                }

                let file = file.take().expect("then closure should only be called once");

                if current_load_on_import.0.as_ref() == Some(&file.meta.id) {
                    current_load_on_import.0 = None;
                    commands.queue(load::Command {
                        source:   load::Source::Parsed(Box::new(file)),
                        on_error: Box::new(|_world, err| bevy::log::error!("Load error: {err}")),
                    });
                }
            }
        });
    }
}

pub const BUILTIN_SCENARIOS: &[&str] = &["maps/tutorial.osav", "maps/demo.osav", "maps/blank.osav"];
pub const DEFAULT_SCENARIO: &str = "omniatc.tutorial";

pub(super) fn import_builtin_scenarios_system(mut commands: Commands) {
    for &scenario in BUILTIN_SCENARIOS {
        commands.queue(ImportScenario(scenario));
    }
}

pub(super) fn warn_failed_default_scenario_system(
    current_load_on_import: Res<CurrentLoadOnImport>,
    current_importing: Res<CurrentImportingScenarios>,
) {
    if let Some(wanted) = &current_load_on_import.0
        && current_importing.0.is_empty()
    {
        bevy::log::error_once!(
            "Unknown default scenario \"{wanted}\" specified in startup options"
        );
    }
}

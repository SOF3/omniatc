use std::borrow::Cow;
use std::collections::VecDeque;
use std::io;
use std::time::SystemTime;

use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Asset, AssetApp, AssetLoader, AssetServer, Assets, Handle};
use bevy::prelude::{Command, Commands, Res, ResMut, Resource, World};
use bevy::reflect::TypePath;
use bevy::utils::{ConditionalSendFuture, HashSet};
use bevy_pkv::PkvStore;
use omniatc_core::store::{self, load};
use serde::{Deserialize, Serialize};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.insert_resource(PkvStore::new("omniatc", "omniatc"));
        app.init_resource::<ImportingMap>();
        app.init_resource::<CurrentSaveKey>();

        app.init_asset::<MapAsset>();
        app.init_asset_loader::<MapAssetLoader>();

        app.add_systems(app::Startup, init_load_system);
        app.add_systems(app::Update, load_imported_map_system);
    }
}

fn init_load_system(mut commands: Commands, store: ResMut<PkvStore>) {
    let LocalIndex { saves } = store.get(LocalIndex::KEY).unwrap_or_default();

    if let Some(Save { key: latest_key, .. }) = saves.front() {
        if let Ok(SaveWrapper { data }) = store.get(latest_key) {
            commands.queue(load::Command {
                source:   load::Source::Raw(data),
                on_error: Box::new(|_world, err| bevy::log::error!("Load error: {err}")),
            });

            return;
        }

        bevy::log::warn!(
            "Last opened level {latest_key:?} is missing, creating a new default level"
        );
    }

    commands.queue(ImportMapCommand(MapRef::Asset("example.osav".into())));
}

#[derive(Default, Serialize, Deserialize)]
struct LocalIndex {
    /// Expect ot be sorted in reverse `modified` order.
    saves: VecDeque<Save>,
}

impl LocalIndex {
    const KEY: &str = "local-index";
}

#[derive(Serialize, Deserialize)]
struct Save {
    key:      String,
    title:    String,
    tags:     HashSet<String>,
    creation: SystemTime,
    modified: SystemTime,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize)]
struct SaveWrapper<'a> {
    #[serde_as(as = "serde_with::IfIsHumanReadable<serde_with::base64::Base64>")]
    data: Cow<'a, [u8]>,
}

pub struct ImportMapCommand(MapRef);

pub enum MapRef {
    Asset(String),
}

impl Command for ImportMapCommand {
    fn apply(self, world: &mut World) {
        let map_asset: Handle<MapAsset> = match self.0 {
            MapRef::Asset(path) => world.resource_mut::<AssetServer>().load(format!("maps/{path}")),
        };

        world.resource_mut::<ImportingMap>().0 = Some(map_asset);
    }
}

#[derive(Default, Resource)]
struct ImportingMap(Option<Handle<MapAsset>>);

fn load_imported_map_system(
    mut commands: Commands,
    assets: Res<Assets<MapAsset>>,
    mut importing_map: ResMut<ImportingMap>,
    mut store: ResMut<PkvStore>,
    mut current_save_key: ResMut<CurrentSaveKey>,
) {
    let Some(ref handle) = importing_map.0 else { return };

    let Some(MapAsset(bytes, file)) = assets.get(handle) else { return };
    importing_map.0 = None;

    let save_key =
        format!("{}", SystemTime::UNIX_EPOCH.elapsed().expect("SystemTime is too old").as_millis());

    let mut index: LocalIndex = store.get(LocalIndex::KEY).unwrap_or_default();
    index.saves.push_front(Save {
        key:      save_key.clone(),
        title:    file.meta.title.clone(),
        tags:     file.meta.tags.iter().cloned().collect(),
        creation: SystemTime::now(),
        modified: SystemTime::now(),
    });

    if let Err(err) = store.set(&save_key, &SaveWrapper { data: Cow::Borrowed(bytes) }) {
        bevy::log::error!("Error saving new file: {err}");
    }

    if let Err(err) = store.set(LocalIndex::KEY, &index) {
        bevy::log::error!("Error writing index: {err}");
    }

    current_save_key.0 = save_key;

    commands.queue(load::Command {
        source:   load::Source::Parsed(Box::new(file.clone())),
        on_error: Box::new(|_world, err| bevy::log::error!("Load error: {err}")),
    });
}

#[derive(Default, Resource)]
pub struct CurrentSaveKey(pub String);

#[derive(Asset, TypePath)]
struct MapAsset(Vec<u8>, store::File);

#[derive(Default)]
struct MapAssetLoader;

impl AssetLoader for MapAssetLoader {
    type Asset = MapAsset;
    type Settings = ();
    type Error = ciborium::de::Error<io::Error>;

    fn load(
        &self,
        reader: &mut dyn asset::io::Reader,
        (): &Self::Settings,
        _load_context: &mut asset::LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>> {
        async {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            ciborium::from_reader(&bytes[..]).map(|file| MapAsset(bytes, file))
        }
    }
}

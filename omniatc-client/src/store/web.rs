use std::cell::OnceCell;
use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;

use anyhow::Context;
use async_lock::Mutex;
use idb::event::VersionChangeEvent;
use idb::{DatabaseEvent, ObjectStoreParams, TransactionMode};
use js_sys::JsString;
use serde::{Deserialize, Serialize};
use serde_with::base64::Base64;
use serde_with::serde_as;
use wasm_bindgen::{JsCast, JsError, JsValue};

use super::{LevelMeta, ScenarioMeta};

#[derive(Default)]
pub struct Impl {
    db: Rc<Mutex<Option<Rc<idb::Database>>>>,
}

const DB_NAME: &str = "omniatc-index";

impl super::Storage for Impl {
    type Error = anyhow::Error;

    fn list_scenarios_by_tag(
        &self,
        tag_key: String,
    ) -> impl Future<Output = anyhow::Result<Vec<ScenarioMeta>>> + 'static {
        let db = self.db.clone();
        async move {
            let db = {
                let mut db = db.lock().await;
                get_db(&mut db).await?
            };

            let tx = db
                .transaction(&["scenario", "scenario_tag"], TransactionMode::ReadOnly)
                .anyhow()
                .context("create transaction")?;
            let ids = {
                let store = tx.object_store("scenario_tag").anyhow().context("get tag store")?;
                let index = store.index("kv").anyhow().context("get kv index")?;
                let cursor = index
                    .open_cursor(None, Some(idb::CursorDirection::Next))
                    .anyhow()
                    .context("open kv cursor")?
                    .await
                    .anyhow()
                    .context("open kv cursor")?;

                let Some(cursor) = cursor else { return Ok(Vec::new()) };

                let mut ids = Vec::new();

                while let Ok(value) = cursor.value() {
                    let entry: TagKv = serde_wasm_bindgen::from_value(value)
                        .anyhow()
                        .context("convert js value to TagKv")?;
                    ids.push(entry.id.clone());
                    cursor
                        .next(None)
                        .anyhow()
                        .context("advance tag cursor")?
                        .await
                        .anyhow()
                        .context("advance tag cursor")?;
                }
                ids
            };

            let store = tx.object_store("scenario").anyhow().context("get scenario store")?;

            let mut output = Vec::with_capacity(ids.len());
            for id in ids {
                let value = store
                    .get(idb::Query::Key(JsString::from(id.as_str()).into()))
                    .anyhow()
                    .context("fetch by id from scenario store")?
                    .await
                    .anyhow()?;
                if let Some(value) = value {
                    let meta: ScenarioMeta = serde_wasm_bindgen::from_value(value)
                        .anyhow()
                        .context("convert js value to ScenarioMeta")?;
                    output.push(meta);
                } else {
                    bevy::log::warn!(
                        "Scenario with id {id:?} referenced from scenario_tag is missing"
                    );
                }
            }

            tx.await.anyhow().context("transaction close")?;
            Ok(output)
        }
    }

    fn load_scenario(
        &self,
        key: String,
    ) -> impl Future<Output = anyhow::Result<Vec<u8>>> + 'static {
        let db = self.db.clone();
        async move {
            let db = {
                let mut db = db.lock().await;
                get_db(&mut db).await?
            };

            let tx = db
                .transaction(&["scenario_data"], TransactionMode::ReadOnly)
                .anyhow()
                .context("create transaction")?;

            let store = tx.object_store("scenario_data").anyhow().context("get scenario store")?;

            let value = store
                .get(idb::Query::Key(JsString::from(key.as_str()).into()))
                .anyhow()
                .context("fetch by id from scenario_data store")?
                .await
                .anyhow()?
                .context("scenario does not eixst")?;
            let data: Data = serde_wasm_bindgen::from_value(value)
                .anyhow()
                .context("convert js value to Data")?;

            tx.await.anyhow().context("transaction close")?;
            Ok(data.data)
        }
    }

    fn insert_scenario(
        &self,
        meta: ScenarioMeta,
        data: Vec<u8>,
        tags: HashMap<String, String>,
    ) -> impl Future<Output = anyhow::Result<()>> + 'static {
        let db = self.db.clone();
        async move {
            let db = {
                let mut db = db.lock().await;
                get_db(&mut db).await?
            };

            let tx = db
                .transaction(
                    &["scenario", "scenario_tag", "scenario_data"],
                    TransactionMode::ReadWrite,
                )
                .anyhow()
                .context("create transaction")?;

            {
                let store =
                    tx.object_store("scenario_data").anyhow().context("get scenario_data store")?;
                let value = serde_wasm_bindgen::to_value(&Data { id: meta.id.to_string(), data })
                    .anyhow()
                    .context("convert Data to js value")?;
                store.add(&value, None).anyhow().context("add scenario to meta store")?;
            }

            {
                let store =
                    tx.object_store("scenario_tag").anyhow().context("get scenario_tag store")?;
                for (tag_key, tag_value) in tags {
                    let value = serde_wasm_bindgen::to_value(&TagKv {
                        id: meta.id.to_string(),
                        tag_key,
                        tag_value,
                    })
                    .anyhow()
                    .context("convert Data to js value")?;
                    store.add(&value, None).anyhow().context("add scenario to meta store")?;
                }
            }

            {
                let store = tx.object_store("scenario").anyhow().context("get scenario store")?;
                let value = serde_wasm_bindgen::to_value(&meta)
                    .anyhow()
                    .context("convert ScenarioMeta to js value")?;
                store.add(&value, None).anyhow().context("add scenario to meta store")?;
            }

            tx.commit().anyhow().context("commit transaction")?;
            Ok(())
        }
    }

    fn list_levels_by_time(
        &self,
        limit: usize,
    ) -> impl Future<Output = anyhow::Result<Vec<LevelMeta>>> + 'static {
        let db = self.db.clone();
        async move {
            let db = {
                let mut db = db.lock().await;
                get_db(&mut db).await?
            };

            let tx = db
                .transaction(&["level"], TransactionMode::ReadOnly)
                .anyhow()
                .context("create transaction")?;
            let store = tx.object_store("level").anyhow().context("get level store")?;
            let index = store.index("modified").anyhow().context("modified index")?;
            let cursor = index
                .open_cursor(None, Some(idb::CursorDirection::Prev))
                .anyhow()
                .context("open modified cursor")?
                .await
                .anyhow()
                .context("open modified cursor")?;

            let Some(cursor) = cursor else { return Ok(Vec::new()) };

            let mut output = Vec::new();

            while let Ok(value) = cursor.value() {
                let entry: LevelMeta = serde_wasm_bindgen::from_value(value)
                    .anyhow()
                    .context("convert js value to LevelMeta")?;
                output.push(entry);
                cursor
                    .next(None)
                    .anyhow()
                    .context("advance level cursor")?
                    .await
                    .anyhow()
                    .context("advance level cursor")?;
            }

            tx.await.anyhow().context("transaction close")?;
            Ok(output)
        }
    }

    fn load_level(&self, key: String) -> impl Future<Output = anyhow::Result<Vec<u8>>> + 'static {
        let db = self.db.clone();
        async move {
            let db = {
                let mut db = db.lock().await;
                get_db(&mut db).await?
            };

            let tx = db
                .transaction(&["level_data"], TransactionMode::ReadOnly)
                .anyhow()
                .context("create transaction")?;

            let store = tx.object_store("level_data").anyhow().context("get level store")?;

            let value = store
                .get(idb::Query::Key(JsString::from(key.as_str()).into()))
                .anyhow()
                .context("fetch by id from level_data store")?
                .await
                .anyhow()?
                .context("level does not eixst")?;
            let data: Data = serde_wasm_bindgen::from_value(value)
                .anyhow()
                .context("convert js value to Data")?;

            tx.await.anyhow().context("transaction close")?;
            Ok(data.data)
        }
    }
}

async fn get_db(db: &mut Option<Rc<idb::Database>>) -> anyhow::Result<Rc<idb::Database>> {
    Ok(if let Some(db) = db {
        Rc::clone(db)
    } else {
        *db = Some(Rc::new(new_db().await?));
        db.as_ref().unwrap().clone()
    })
}

async fn new_db() -> anyhow::Result<idb::Database> {
    let factory = idb::Factory::new().anyhow().context("new idb factory")?;
    let mut open = factory.open("omniatc", Some(1)).anyhow().context("open omniatc idb")?;

    open.on_upgrade_needed(|event| {
        if let Err(err) = migrate_db(event) {
            bevy::log::error!("migrate db error: {err}");
        }
    });

    open.await.anyhow().context("open idb")
}

fn migrate_db(event: VersionChangeEvent) -> anyhow::Result<()> {
    let database = event.database().unwrap();

    let scenario_store = database
        .create_object_store("scenario", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .anyhow()
        .context("create scenario store")?;
    scenario_store
        .create_index("created", idb::KeyPath::new_single("created"), None)
        .anyhow()
        .context("create scenario.created index")?;

    let scenario_tag_store = database
        .create_object_store("scenario_tag", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_array(["id", "tag_key"])));
            params
        })
        .anyhow()
        .context("create scenario_tag store")?;
    scenario_tag_store
        .create_index("kv", idb::KeyPath::new_single("created"), None)
        .anyhow()
        .context("create scenario_tag.kv index")?;

    database
        .create_object_store("scenario_data", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .anyhow()
        .context("create scenario_data store")?;

    let level_store = database
        .create_object_store("level", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .anyhow()
        .context("create level store")?;
    level_store
        .create_index("created", idb::KeyPath::new_single("created"), None)
        .anyhow()
        .context("create level.created index")?;
    level_store
        .create_index("modified", idb::KeyPath::new_single("modified"), None)
        .anyhow()
        .context("create level.modified index")?;

    let level_tag_store = database
        .create_object_store("level_tag", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_array(["id", "tag_key"])));
            params
        })
        .anyhow()
        .context("create level_tag store")?;
    level_tag_store
        .create_index("kv", idb::KeyPath::new_single("created"), None)
        .anyhow()
        .context("create level_tag.kv index")?;

    database
        .create_object_store("level_data", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .anyhow()
        .context("create level_data store")?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TagKv {
    id:        String,
    tag_key:   String,
    tag_value: String,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
struct Data {
    id:   String,
    #[serde_as(as = "Base64")]
    data: Vec<u8>,
}

trait JsValueResultExt<T> {
    fn anyhow(self) -> anyhow::Result<T>;
}

impl<T> JsValueResultExt<T> for Result<T, JsValue> {
    fn anyhow(self) -> anyhow::Result<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => {
                if let Some(s) = err.dyn_ref::<JsString>() {
                    Err(anyhow::anyhow!("{}", String::from(s)))
                } else {
                    Err(anyhow::anyhow!("{err:?}"))
                }
            }
        }
    }
}

impl<T> JsValueResultExt<T> for Result<T, serde_wasm_bindgen::Error> {
    fn anyhow(self) -> anyhow::Result<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(anyhow::anyhow!("{err}")),
        }
    }
}

impl<T> JsValueResultExt<T> for Result<T, idb::Error> {
    fn anyhow(self) -> anyhow::Result<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(anyhow::anyhow!("{err}")),
        }
    }
}

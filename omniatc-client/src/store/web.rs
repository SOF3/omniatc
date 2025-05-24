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
                .ok()
                .context("create transaction")?;
            let ids = {
                let store = tx.object_store("scenario_tag").ok().context("get tag store")?;
                let index = store.index("kv").ok().context("get kv index")?;
                let cursor = index
                    .open_cursor(None, Some(idb::CursorDirection::Next))
                    .ok()
                    .context("open kv cursor")?
                    .await
                    .ok()
                    .context("open kv cursor")?
                    .context("kv cursor is missing")?;

                let mut ids = Vec::new();

                while let Ok(value) = cursor.value() {
                    let entry: TagKv = serde_wasm_bindgen::from_value(value)
                        .ok()
                        .context("convert js value to TagKv")?;
                    ids.push(entry.id.clone());
                    cursor
                        .next(None)
                        .ok()
                        .context("advance tag cursor")?
                        .await
                        .ok()
                        .context("advance tag cursor")?;
                }
                ids
            };

            let store = tx.object_store("scenario").ok().context("get scenario store")?;

            let mut output = Vec::with_capacity(ids.len());
            for id in ids {
                let value = store
                    .get(idb::Query::Key(JsString::from(id.as_str()).into()))
                    .ok()
                    .context("fetch by id from scenario store")?
                    .await
                    .ok()
                    .flatten();
                if let Some(value) = value {
                    let meta: ScenarioMeta = serde_wasm_bindgen::from_value(value)
                        .ok()
                        .context("convert js value to ScenarioMeta")?;
                    output.push(meta);
                } else {
                    bevy::log::warn!(
                        "Scenario with id {id:?} referenced from scenario_tag is missing"
                    );
                }
            }

            tx.await.ok().context("transaction close")?;
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
                .ok()
                .context("create transaction")?;

            let store = tx.object_store("scenario_data").ok().context("get scenario store")?;

            let value = store
                .get(idb::Query::Key(JsString::from(key.as_str()).into()))
                .ok()
                .context("fetch by id from scenario_data store")?
                .await
                .ok()
                .flatten()
                .context("scenario does not eixst")?;
            let data: Data =
                serde_wasm_bindgen::from_value(value).ok().context("convert js value to Data")?;

            tx.await.ok().context("transaction close")?;
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
                .ok()
                .context("create transaction")?;

            {
                let store =
                    tx.object_store("scenario_data").ok().context("get scenario_data store")?;
                let value = serde_wasm_bindgen::to_value(&Data { id: meta.id.to_string(), data })
                    .ok()
                    .context("convert Data to js value")?;
                store.add(&value, None).ok().context("add scenario to meta store")?;
            }

            {
                let store =
                    tx.object_store("scenario_tag").ok().context("get scenario_tag store")?;
                for (tag_key, tag_value) in tags {
                    let value = serde_wasm_bindgen::to_value(&TagKv {
                        id: meta.id.to_string(),
                        tag_key,
                        tag_value,
                    })
                    .ok()
                    .context("convert Data to js value")?;
                    store.add(&value, None).ok().context("add scenario to meta store")?;
                }
            }

            {
                let store = tx.object_store("scenario").ok().context("get scenario store")?;
                let value = serde_wasm_bindgen::to_value(&meta)
                    .ok()
                    .context("convert ScenarioMeta to js value")?;
                store.add(&value, None).ok().context("add scenario to meta store")?;
            }

            tx.commit().ok().context("commit transaction")?;
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
                .ok()
                .context("create transaction")?;
            let store = tx.object_store("level").ok().context("get level store")?;
            let index = store.index("modified").ok().context("modified index")?;
            let cursor = index
                .open_cursor(None, Some(idb::CursorDirection::Prev))
                .ok()
                .context("open modified cursor")?
                .await
                .ok()
                .context("open modified cursor")?
                .context("modified cursor is missing")?;

            let mut output = Vec::new();

            while let Ok(value) = cursor.value() {
                let entry: LevelMeta = serde_wasm_bindgen::from_value(value)
                    .ok()
                    .context("convert js value to LevelMeta")?;
                output.push(entry);
                cursor
                    .next(None)
                    .ok()
                    .context("advance level cursor")?
                    .await
                    .ok()
                    .context("advance level cursor")?;
            }

            tx.await.ok().context("transaction close")?;
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
                .ok()
                .context("create transaction")?;

            let store = tx.object_store("level_data").ok().context("get level store")?;

            let value = store
                .get(idb::Query::Key(JsString::from(key.as_str()).into()))
                .ok()
                .context("fetch by id from level_data store")?
                .await
                .ok()
                .flatten()
                .context("level does not eixst")?;
            let data: Data =
                serde_wasm_bindgen::from_value(value).ok().context("convert js value to Data")?;

            tx.await.ok().context("transaction close")?;
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
    let factory = idb::Factory::new().ok().context("new idb factory")?;
    let mut open = factory.open("omniatc", Some(1)).ok().context("open omniatc idb")?;

    open.on_upgrade_needed(|event| {
        if let Err(err) = migrate_db(event) {
            bevy::log::error!("migrate db error: {err}");
        }
    });

    open.await.ok().context("open idb")
}

fn migrate_db(event: VersionChangeEvent) -> anyhow::Result<()> {
    let database = event.database().unwrap();

    let scenario_store = database
        .create_object_store("scenario", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .ok()
        .context("create scenario store")?;
    scenario_store
        .create_index("created", idb::KeyPath::new_single("created"), None)
        .ok()
        .context("create scenario.created index")?;

    let scenario_tag_store = database
        .create_object_store("scenario_tag", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_array(["id", "tag_key"])));
            params
        })
        .ok()
        .context("create scenario_tag store")?;
    scenario_tag_store
        .create_index("kv", idb::KeyPath::new_single("created"), None)
        .ok()
        .context("create scenario_tag.kv index")?;

    database
        .create_object_store("scenario_data", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .ok()
        .context("create scenario_data store")?;

    let level_store = database
        .create_object_store("level", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .ok()
        .context("create level store")?;
    level_store
        .create_index("created", idb::KeyPath::new_single("created"), None)
        .ok()
        .context("create level.created index")?;
    level_store
        .create_index("modified", idb::KeyPath::new_single("modified"), None)
        .ok()
        .context("create level.modified index")?;

    let level_tag_store = database
        .create_object_store("level_tag", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_array(["id", "tag_key"])));
            params
        })
        .ok()
        .context("create level_tag store")?;
    level_tag_store
        .create_index("kv", idb::KeyPath::new_single("created"), None)
        .ok()
        .context("create level_tag.kv index")?;

    database
        .create_object_store("level_data", {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("id")));
            params
        })
        .ok()
        .context("create level_data store")?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TagKv {
    id:        String,
    tag_key:   String,
    tag_value: String,
}

#[derive(Serialize, Deserialize)]
#[serde_as]
struct Data {
    id:   String,
    #[serde_as(as = "Base64")]
    data: Vec<u8>,
}

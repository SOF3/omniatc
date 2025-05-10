use std::cell::OnceCell;
use std::rc::Rc;

use anyhow::Context;

pub struct Impl {}

const DB_NAME: &str = "omniatc-index";

impl super::Storage for Impl {
    type Error = anyhow::Error;

    fn list_scenarios_by_tag(
        &self,
        tag_key: String,
    ) -> impl Future<Output = anyhow::Result<Vec<ScenarioMeta>>> + 'static {
        todo!()
    }

    fn load_scenario(
        &self,
        key: String,
    ) -> impl Future<Output = anyhow::Result<Vec<u8>>> + 'static {
        todo!()
    }

    fn insert_scenario(
        &self,
        meta: ScenarioMeta,
        data: Vec<u8>,
        tags: HashMap<String, String>,
    ) -> impl Future<Output = anyhow::Result<()>> + 'static {
        todo!()
    }

    fn list_levels_by_time(
        &self,
        limit: usize,
    ) -> impl Future<Output = anyhow::Result<Vec<LevelMeta>>> + 'static {
        todo!()
    }

    fn load_level(&self, key: String) -> impl Future<Output = anyhow::Result<Vec<u8>>> + 'static {
        todo!()
    }
}

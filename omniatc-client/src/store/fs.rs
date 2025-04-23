use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Context as _;
use bevy::ecs::resource::Resource;
use parking_lot::Mutex;

use super::{LevelMeta, ScenarioMeta};

fn data_path() -> Option<PathBuf> {
    let mut path = dirs::data_dir()?;
    path.push("omniatc");
    Some(path)
}

fn index_path() -> Option<PathBuf> {
    let mut path = data_path()?;
    path.push("index.db");
    Some(path)
}

#[derive(Resource)]
pub struct Impl {
    db: Mutex<rusqlite::Connection>,
}

impl Impl {
    pub fn try_new() -> anyhow::Result<Self> {
        std::fs::create_dir_all(data_path().context("cannot find data path")?)?;

        let db = rusqlite::Connection::open(index_path().context("cannot find data path")?)
            .context("cannot open database")?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS scenario (
            id TEXT PRIMARY KEY,
            title TEXT,
            created INTEGER,
            data BLOB
        )",
            (),
        )
        .context("prepare scenario table")?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS scenario_tag (
            id TEXT,
            tag_key TEXT,
            tag_value TEXT,
            PRIMARY KEY (id, tag_key)
        )",
            (),
        )
        .context("prepare scenario_tag table")?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS level (
            id TEXT PRIMARY KEY,
            title TEXT,
            created INTEGER,
            modified INTEGER,
            data BLOB
        )",
            (),
        )
        .context("prepare scenario table")?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS level_tag (
            id TEXT,
            tag_key TEXT,
            tag_value TEXT,
            PRIMARY KEY (id, tag_key)
        )",
            (),
        )
        .context("prepare scenario_tag table")?;

        Ok(Self { db: Mutex::new(db) })
    }
}

impl super::Storage for Impl {
    type Error = anyhow::Error;

    fn list_scenarios_by_tag(&mut self, tag_key: &str) -> anyhow::Result<Vec<ScenarioMeta>> {
        let db = self.db.get_mut();
        let mut stmt = db
            .prepare(
                "SELECT id, title, created FROM scenario LEFT JOIN scenario_tag USING (id) WHERE \
                 scenario_tag.tag_key = ? ORDER BY scenario_tag.tag_value",
            )
            .context("prepare scenario list statement")?;
        let scenarios = stmt
            .query_map((tag_key,), |row| {
                Ok(ScenarioMeta {
                    key:     row.get(0)?,
                    title:   row.get(1)?,
                    created: SystemTime::UNIX_EPOCH + Duration::from_millis(row.get(2)?),
                })
            })
            .context("query scenario list")?;
        scenarios
            .map(|result| result.context("convert scenario row"))
            .collect::<anyhow::Result<Vec<_>>>()
    }

    fn load_scenario(&mut self, key: &str) -> anyhow::Result<Vec<u8>> {
        let db = self.db.get_mut();
        let mut stmt = db
            .prepare("SELECT data FROM scenario WHERE id = ?")
            .context("prepare scenario select query")?;
        stmt.query_row((key,), |row| row.get(0)).context("query scenario data")
    }

    fn insert_scenario(
        &mut self,
        meta: ScenarioMeta,
        data: &[u8],
        tags: &HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let db = self.db.get_mut();
        db.execute(
            "INSERT OR REPLACE INTO scenario (id, title, created, data) VALUES (?, ?, ?, ?)",
            (
                &meta.key,
                &meta.title,
                u64::try_from(
                    meta.created
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .expect("system time is too old")
                        .as_millis(),
                )
                .expect("system time is too late"),
                data,
            ),
        )
        .context("insert scalar data")?;

        db.execute("DELETE FROM scenario_tag WHERE id = ?", (&meta.key,))
            .context("delete old tags")?;
        for (key, value) in tags {
            db.execute(
                "INSERT INTO scenario_tag (id, tag_key, tag_value) VALUES (?, ?, ?)",
                (&meta.key, key, value),
            )
            .context("insert tag")?;
        }

        Ok(())
    }

    fn list_levels_by_time(&mut self, limit: usize) -> anyhow::Result<Vec<LevelMeta>> {
        let db = self.db.get_mut();
        let mut stmt = db
            .prepare(
                "SELECT id, title, created, modified FROM level ORDER BY modified DESC LIMIT ?",
            )
            .context("prepare level list statement")?;
        let levels = stmt
            .query_map((limit,), |row| {
                Ok(LevelMeta {
                    key:      row.get(0)?,
                    title:    row.get(1)?,
                    created:  SystemTime::UNIX_EPOCH + Duration::from_millis(row.get(2)?),
                    modified: SystemTime::UNIX_EPOCH + Duration::from_millis(row.get(3)?),
                })
            })
            .context("query level list")?;
        levels.map(|result| result.context("convert level row")).collect::<anyhow::Result<Vec<_>>>()
    }

    fn load_level(&mut self, key: &str) -> anyhow::Result<Vec<u8>> {
        let db = self.db.get_mut();
        let mut stmt = db
            .prepare("SELECT data FROM level WHERE id = ?")
            .context("prepare level select query")?;
        stmt.query_row((key,), |row| row.get(0)).context("query level data")
    }
}

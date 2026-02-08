use std::cell::OnceCell;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Context as _;
use jiff::{SignedDuration, Timestamp};

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

#[derive(Default)]
pub struct Impl {
    db: Rc<OnceCell<rusqlite::Connection>>,
}

fn new_db() -> anyhow::Result<rusqlite::Connection> {
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

    Ok(db)
}

impl super::Storage for Impl {
    type Error = anyhow::Error;

    fn list_scenarios_by_tag(
        &self,
        tag_key: String,
    ) -> impl Future<Output = anyhow::Result<Vec<ScenarioMeta>>> + 'static {
        let db = self.db.clone();
        let run = (|| {
            let db = get_db(&db)?;
            let mut stmt = db
                .prepare(
                    "SELECT id, title, created FROM scenario LEFT JOIN scenario_tag USING (id) \
                     WHERE scenario_tag.tag_key = ? ORDER BY scenario_tag.tag_value",
                )
                .context("prepare scenario list statement")?;
            let scenarios = stmt
                .query_map((tag_key,), |row| {
                    Ok(ScenarioMeta {
                        id:      row.get(0)?,
                        title:   row.get(1)?,
                        created: Timestamp::UNIX_EPOCH + SignedDuration::from_millis(row.get(2)?),
                    })
                })
                .context("query scenario list")?;
            scenarios
                .map(|result| result.context("convert scenario row"))
                .collect::<anyhow::Result<Vec<_>>>()
        })();
        async move { run }
    }

    fn load_scenario(
        &self,
        key: String,
    ) -> impl Future<Output = anyhow::Result<Vec<u8>>> + 'static {
        let db = self.db.clone();
        let run = (|| {
            let db = get_db(&db)?;
            let mut stmt = db
                .prepare("SELECT data FROM scenario WHERE id = ?")
                .context("prepare scenario select query")?;
            stmt.query_row((key,), |row| row.get(0)).context("query scenario data")
        })();
        async move { run }
    }

    fn insert_scenario(
        &self,
        meta: ScenarioMeta,
        data: Vec<u8>,
        tags: HashMap<String, String>,
    ) -> impl Future<Output = anyhow::Result<()>> + 'static {
        let db = self.db.clone();
        let run = (|| {
            let db = get_db(&db)?;
            db.execute(
                "INSERT OR REPLACE INTO scenario (id, title, created, data) VALUES (?, ?, ?, ?)",
                (
                    &meta.id,
                    &meta.title,
                    i64::try_from(meta.created.duration_since(Timestamp::UNIX_EPOCH).as_millis())
                        .expect("system time is too late"),
                    data,
                ),
            )
            .context("insert scalar data")?;

            db.execute("DELETE FROM scenario_tag WHERE id = ?", (&meta.id,))
                .context("delete old tags")?;
            for (key, value) in tags {
                db.execute(
                    "INSERT INTO scenario_tag (id, tag_key, tag_value) VALUES (?, ?, ?)",
                    (&meta.id, key, value),
                )
                .context("insert tag")?;
            }

            Ok(())
        })();
        async move { run }
    }

    fn list_levels_by_time(
        &self,
        limit: usize,
    ) -> impl Future<Output = anyhow::Result<Vec<LevelMeta>>> + 'static {
        let db = self.db.clone();
        let run = (|| {
            let db = get_db(&db)?;
            let mut stmt = db
                .prepare(
                    "SELECT id, title, created, modified FROM level ORDER BY modified DESC LIMIT ?",
                )
                .context("prepare level list statement")?;
            let levels = stmt
                .query_map(
                    (i64::try_from(limit).context("attempt to read too many levels")?,),
                    |row| {
                        Ok(LevelMeta {
                            id:       row.get(0)?,
                            title:    row.get(1)?,
                            created:  Timestamp::UNIX_EPOCH
                                + SignedDuration::from_millis(row.get(2)?),
                            modified: Timestamp::UNIX_EPOCH
                                + SignedDuration::from_millis(row.get(3)?),
                        })
                    },
                )
                .context("query level list")?;
            levels
                .map(|result| result.context("convert level row"))
                .collect::<anyhow::Result<Vec<_>>>()
        })();
        async move { run }
    }

    fn load_level(&self, key: String) -> impl Future<Output = anyhow::Result<Vec<u8>>> + 'static {
        let db = self.db.clone();
        let run = (|| {
            let db = get_db(&db)?;
            let mut stmt = db
                .prepare("SELECT data FROM level WHERE id = ?")
                .context("prepare level select query")?;
            stmt.query_row((key,), |row| row.get(0)).context("query level data")
        })();
        async move { run }
    }
}

fn get_db(cell: &Rc<OnceCell<rusqlite::Connection>>) -> anyhow::Result<&rusqlite::Connection> {
    if let Some(db) = cell.get() {
        Ok(db)
    } else {
        let db = new_db()?;
        Ok(cell.get_or_init(move || db))
    }
}

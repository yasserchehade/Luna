use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Clone)]
pub struct SettingsStore {
    database: PathBuf,
}

impl SettingsStore {
    pub fn open(database: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let store = Self {
            database: database.as_ref().to_owned(),
        };
        let connection = store.connect()?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS device_settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            )",
            [],
        )?;
        Ok(store)
    }

    pub fn set(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.connect()?.execute(
            "INSERT INTO device_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.connect()?
            .query_row(
                "SELECT value FROM device_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        Connection::open(&self.database)
    }
}

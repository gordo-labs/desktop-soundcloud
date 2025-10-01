use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::Value;
use tauri::AppHandle;

#[derive(Debug)]
pub enum LibraryError {
    AppDataDirUnavailable,
    Io(std::io::Error),
    Database(rusqlite::Error),
    Serialization(serde_json::Error),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::AppDataDirUnavailable => {
                write!(f, "unable to resolve application data directory")
            }
            LibraryError::Io(error) => write!(f, "filesystem error: {error}"),
            LibraryError::Database(error) => write!(f, "database error: {error}"),
            LibraryError::Serialization(error) => write!(f, "serialization error: {error}"),
        }
    }
}

impl Error for LibraryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            LibraryError::AppDataDirUnavailable => None,
            LibraryError::Io(error) => Some(error),
            LibraryError::Database(error) => Some(error),
            LibraryError::Serialization(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for LibraryError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for LibraryError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<serde_json::Error> for LibraryError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}

#[derive(Debug, Deserialize)]
pub struct TrackRecord {
    pub track_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub discogs_payload: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SoundcloudSourceRecord {
    pub track_id: String,
    pub soundcloud_id: String,
    #[serde(default)]
    pub permalink_url: Option<String>,
    pub raw_payload: Value,
}

fn default_available() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct LocalAssetRecord {
    pub track_id: String,
    pub location: String,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default = "default_available")]
    pub available: bool,
    #[serde(default)]
    pub rekordbox_cues: Option<Value>,
}

pub struct LibraryStore {
    connection: Connection,
}

impl LibraryStore {
    pub fn initialize(app: &AppHandle) -> Result<Self, LibraryError> {
        let mut database_path = resolve_database_path(app)?;
        fs::create_dir_all(&database_path)?;
        database_path.push("library.sqlite3");

        let connection = Connection::open(database_path)?;
        let store = Self { connection };
        store.apply_migrations()?;
        store.enable_foreign_keys()?;
        Ok(store)
    }

    fn enable_foreign_keys(&self) -> Result<(), LibraryError> {
        self.connection.execute("PRAGMA foreign_keys = ON;", [])?;
        Ok(())
    }

    fn apply_migrations(&self) -> Result<(), LibraryError> {
        self.connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                title TEXT,
                artist TEXT,
                album TEXT,
                discogs_payload TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS soundcloud_sources (
                track_id TEXT PRIMARY KEY,
                soundcloud_id TEXT NOT NULL,
                permalink_url TEXT,
                raw_payload TEXT NOT NULL,
                fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS rekordbox_sources (
                track_id TEXT PRIMARY KEY,
                raw_payload TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS local_assets (
                track_id TEXT PRIMARY KEY,
                location TEXT NOT NULL,
                checksum TEXT,
                available INTEGER NOT NULL DEFAULT 1,
                recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_track(&self, record: &TrackRecord) -> Result<(), LibraryError> {
        let discogs_payload = record
            .discogs_payload
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.connection.execute(
            r#"
            INSERT INTO tracks (id, title, artist, album, discogs_payload)
            VALUES (:id, :title, :artist, :album, :discogs_payload)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                discogs_payload = excluded.discogs_payload,
                updated_at = datetime('now');
            "#,
            rusqlite::named_params! {
                ":id": record.track_id,
                ":title": record.title,
                ":artist": record.artist,
                ":album": record.album,
                ":discogs_payload": discogs_payload,
            },
        )?;

        Ok(())
    }

    pub fn link_soundcloud_source(
        &self,
        record: &SoundcloudSourceRecord,
    ) -> Result<(), LibraryError> {
        let payload = serde_json::to_string(&record.raw_payload)?;
        self.ensure_track(&record.track_id)?;

        self.connection.execute(
            r#"
            INSERT INTO soundcloud_sources (track_id, soundcloud_id, permalink_url, raw_payload)
            VALUES (:track_id, :soundcloud_id, :permalink_url, :raw_payload)
            ON CONFLICT(track_id) DO UPDATE SET
                soundcloud_id = excluded.soundcloud_id,
                permalink_url = excluded.permalink_url,
                raw_payload = excluded.raw_payload,
                fetched_at = datetime('now');
            "#,
            rusqlite::named_params! {
                ":track_id": record.track_id,
                ":soundcloud_id": record.soundcloud_id,
                ":permalink_url": record.permalink_url,
                ":raw_payload": payload,
            },
        )?;

        Ok(())
    }

    pub fn record_local_asset(&self, record: &LocalAssetRecord) -> Result<(), LibraryError> {
        self.ensure_track(&record.track_id)?;
        self.connection.execute(
            r#"
            INSERT INTO local_assets (track_id, location, checksum, available)
            VALUES (:track_id, :location, :checksum, :available)
            ON CONFLICT(track_id) DO UPDATE SET
                location = excluded.location,
                checksum = excluded.checksum,
                available = excluded.available,
                recorded_at = datetime('now');
            "#,
            rusqlite::named_params! {
                ":track_id": record.track_id,
                ":location": record.location,
                ":checksum": record.checksum,
                ":available": i64::from(record.available),
            },
        )?;

        if let Some(cues) = &record.rekordbox_cues {
            let payload = serde_json::to_string(cues)?;
            self.connection.execute(
                r#"
                INSERT INTO rekordbox_sources (track_id, raw_payload)
                VALUES (:track_id, :raw_payload)
                ON CONFLICT(track_id) DO UPDATE SET
                    raw_payload = excluded.raw_payload,
                    updated_at = datetime('now');
                "#,
                rusqlite::named_params! {
                    ":track_id": record.track_id,
                    ":raw_payload": payload,
                },
            )?;
        }

        Ok(())
    }

    pub fn list_missing_assets(&self) -> Result<Vec<String>, LibraryError> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT tracks.id
            FROM tracks
            LEFT JOIN local_assets ON local_assets.track_id = tracks.id
            WHERE local_assets.track_id IS NULL OR local_assets.available = 0
            ORDER BY tracks.id ASC;
            "#,
        )?;

        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn ensure_track(&self, track_id: &str) -> Result<(), LibraryError> {
        self.connection.execute(
            "INSERT OR IGNORE INTO tracks (id) VALUES (?1);",
            params![track_id],
        )?;
        Ok(())
    }
}

fn resolve_database_path(app: &AppHandle) -> Result<PathBuf, LibraryError> {
    let resolver = app.path_resolver();
    let base = resolver
        .app_data_dir()
        .ok_or(LibraryError::AppDataDirUnavailable)?;
    Ok(base)
}

impl From<bool> for i64 {
    fn from(value: bool) -> Self {
        if value {
            1
        } else {
            0
        }
    }
}

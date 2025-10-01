use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::rekordbox::RekordboxTrack;
use rusqlite::{params, Connection, ErrorCode};
use serde::Deserialize;
use serde_json::{json, Value};
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
    pub duration_ms: Option<i64>,
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
                duration_ms INTEGER,
                recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS rekordbox_mappings (
                rekordbox_id TEXT PRIMARY KEY,
                track_id TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );
            "#,
        )?;

        if let Err(error) = self.connection.execute(
            "ALTER TABLE local_assets ADD COLUMN duration_ms INTEGER;",
            [],
        ) {
            match error {
                rusqlite::Error::SqliteFailure(ref failure, _)
                    if failure.code == ErrorCode::DuplicateColumnName => {}
                _ => return Err(error.into()),
            }
        }
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

    pub fn sync_soundcloud_track(
        &self,
        track: &TrackRecord,
        source: &SoundcloudSourceRecord,
    ) -> Result<(), LibraryError> {
        self.upsert_track(track)?;
        self.link_soundcloud_source(source)?;
        Ok(())
    }

    pub fn record_discogs_success(
        &self,
        track_id: &str,
        query: &str,
        release: &Value,
        confidence: f32,
    ) -> Result<(), LibraryError> {
        let payload = json!({
            "status": "success",
            "query": query,
            "confidence": confidence,
            "release": release,
        });
        self.update_discogs_payload(track_id, &payload)
    }

    pub fn record_discogs_ambiguity(
        &self,
        track_id: &str,
        query: &str,
        candidates: &[Value],
    ) -> Result<(), LibraryError> {
        let payload = json!({
            "status": "ambiguous",
            "query": query,
            "candidates": candidates,
        });
        self.update_discogs_payload(track_id, &payload)
    }

    pub fn record_discogs_failure(
        &self,
        track_id: &str,
        query: &str,
        reason: &str,
    ) -> Result<(), LibraryError> {
        let payload = json!({
            "status": "error",
            "query": query,
            "reason": reason,
        });
        self.update_discogs_payload(track_id, &payload)
    }

    pub fn record_local_asset(&self, record: &LocalAssetRecord) -> Result<(), LibraryError> {
        self.ensure_track(&record.track_id)?;
        self.connection.execute(
            r#"
            INSERT INTO local_assets (track_id, location, checksum, available, duration_ms)
            VALUES (:track_id, :location, :checksum, :available, :duration_ms)
            ON CONFLICT(track_id) DO UPDATE SET
                location = excluded.location,
                checksum = excluded.checksum,
                available = excluded.available,
                duration_ms = excluded.duration_ms,
                recorded_at = datetime('now');
            "#,
            rusqlite::named_params! {
                ":track_id": record.track_id,
                ":location": record.location,
                ":checksum": record.checksum,
                ":available": i64::from(record.available),
                ":duration_ms": record.duration_ms,
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

    pub fn sync_rekordbox_tracks(&self, tracks: &[RekordboxTrack]) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;

        let mut existing_statement =
            transaction.prepare("SELECT rekordbox_id, track_id FROM rekordbox_mappings")?;
        let existing_rows = existing_statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut existing_map: HashMap<String, String> = HashMap::new();
        for row in existing_rows {
            let (rekordbox_id, track_id) = row?;
            existing_map.insert(rekordbox_id, track_id);
        }
        let mut stale_map = existing_map.clone();

        for track in tracks {
            let track_id = existing_map
                .get(&track.rekordbox_id)
                .cloned()
                .unwrap_or_else(|| format!("rekordbox:{}", track.rekordbox_id));
            existing_map.insert(track.rekordbox_id.clone(), track_id.clone());
            stale_map.remove(&track.rekordbox_id);

            transaction.execute(
                r#"
                INSERT INTO tracks (id, title, artist, album)
                VALUES (:id, :title, :artist, :album)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    artist = excluded.artist,
                    album = excluded.album,
                    updated_at = datetime('now');
                "#,
                rusqlite::named_params! {
                    ":id": &track_id,
                    ":title": track.title.as_ref(),
                    ":artist": track.artist.as_ref(),
                    ":album": track.album.as_ref(),
                },
            )?;

            transaction.execute(
                r#"
                INSERT INTO rekordbox_mappings (rekordbox_id, track_id)
                VALUES (:rekordbox_id, :track_id)
                ON CONFLICT(rekordbox_id) DO UPDATE SET
                    track_id = excluded.track_id,
                    updated_at = datetime('now');
                "#,
                rusqlite::named_params! {
                    ":rekordbox_id": &track.rekordbox_id,
                    ":track_id": &track_id,
                },
            )?;

            if let Some(location) = track.location.clone().or_else(|| {
                track
                    .normalized_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
            }) {
                transaction.execute(
                    r#"
                    INSERT INTO local_assets (track_id, location, checksum, available, duration_ms)
                    VALUES (:track_id, :location, :checksum, :available, :duration_ms)
                    ON CONFLICT(track_id) DO UPDATE SET
                        location = excluded.location,
                        checksum = excluded.checksum,
                        available = excluded.available,
                        duration_ms = excluded.duration_ms,
                        recorded_at = datetime('now');
                    "#,
                    rusqlite::named_params! {
                        ":track_id": &track_id,
                        ":location": location,
                        ":checksum": track.checksum.as_ref(),
                        ":available": i64::from(track.available),
                        ":duration_ms": track.duration_ms.map(|value| value as i64),
                    },
                )?;
            }

            let raw_payload = serde_json::to_string(&json!({
                "rekordbox_id": track.rekordbox_id,
                "track_reference": track.track_reference,
                "track_id": track_id,
                "title": track.title,
                "artist": track.artist,
                "album": track.album,
                "location": track.location,
                "normalized_path": track.normalized_path,
                "checksum": track.checksum,
                "duration_ms": track.duration_ms,
                "available": track.available,
                "cues": track.cues,
            }))?;

            transaction.execute(
                r#"
                INSERT INTO rekordbox_sources (track_id, raw_payload)
                VALUES (:track_id, :raw_payload)
                ON CONFLICT(track_id) DO UPDATE SET
                    raw_payload = excluded.raw_payload,
                    updated_at = datetime('now');
                "#,
                rusqlite::named_params! {
                    ":track_id": &track_id,
                    ":raw_payload": raw_payload,
                },
            )?;
        }

        for (_rekordbox_id, track_id) in stale_map {
            transaction.execute(
                "DELETE FROM tracks WHERE id = :track_id;",
                rusqlite::named_params! { ":track_id": track_id },
            )?;
        }

        transaction.commit()?;
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

    fn update_discogs_payload(&self, track_id: &str, payload: &Value) -> Result<(), LibraryError> {
        self.ensure_track(track_id)?;
        let payload = Some(serde_json::to_string(payload)?);
        self.connection.execute(
            r#"
            UPDATE tracks
            SET discogs_payload = :payload,
                updated_at = datetime('now')
            WHERE id = :track_id;
            "#,
            rusqlite::named_params! {
                ":track_id": track_id,
                ":payload": payload,
            },
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

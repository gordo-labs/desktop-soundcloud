use std::collections::HashMap;
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
    pub discogs_release_id: Option<String>,
    #[serde(default)]
    pub discogs_confidence: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub enum DiscogsMatchStatus {
    Success,
    Ambiguous,
    Error,
}

impl DiscogsMatchStatus {
    fn as_str(&self) -> &'static str {
        match self {
            DiscogsMatchStatus::Success => "success",
            DiscogsMatchStatus::Ambiguous => "ambiguous",
            DiscogsMatchStatus::Error => "error",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "success" => DiscogsMatchStatus::Success,
            "ambiguous" => DiscogsMatchStatus::Ambiguous,
            _ => DiscogsMatchStatus::Error,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscogsMatchRecord {
    pub track_id: String,
    pub release_id: Option<String>,
    pub confidence: Option<f32>,
    pub status: DiscogsMatchStatus,
    pub query: Option<String>,
    pub message: Option<String>,
    pub checked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiscogsCandidateRecord {
    pub match_id: String,
    pub release_id: Option<String>,
    pub score: Option<f32>,
    pub raw_payload: Value,
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
                discogs_release_id TEXT,
                discogs_confidence REAL,
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

            CREATE TABLE IF NOT EXISTS discogs_matches (
                track_id TEXT PRIMARY KEY,
                release_id TEXT,
                confidence REAL,
                status TEXT NOT NULL,
                query TEXT,
                message TEXT,
                checked_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS discogs_candidates (
                match_id TEXT NOT NULL,
                release_id TEXT,
                score REAL,
                raw_payload TEXT NOT NULL,
                FOREIGN KEY(match_id) REFERENCES discogs_matches(track_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS discogs_matches_release_idx ON discogs_matches(release_id);
            CREATE INDEX IF NOT EXISTS discogs_matches_status_idx ON discogs_matches(status);
            CREATE INDEX IF NOT EXISTS discogs_candidates_match_idx ON discogs_candidates(match_id);
            CREATE INDEX IF NOT EXISTS discogs_candidates_release_idx ON discogs_candidates(release_id);
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

        if let Err(error) = self
            .connection
            .execute("ALTER TABLE tracks ADD COLUMN discogs_release_id TEXT;", [])
        {
            match error {
                rusqlite::Error::SqliteFailure(ref failure, _)
                    if failure.code == ErrorCode::DuplicateColumnName => {}
                _ => return Err(error.into()),
            }
        }

        if let Err(error) = self
            .connection
            .execute("ALTER TABLE tracks ADD COLUMN discogs_confidence REAL;", [])
        {
            match error {
                rusqlite::Error::SqliteFailure(ref failure, _)
                    if failure.code == ErrorCode::DuplicateColumnName => {}
                _ => return Err(error.into()),
            }
        }

        self.migrate_discogs_payloads()?;
        Ok(())
    }

    pub fn upsert_track(&self, record: &TrackRecord) -> Result<(), LibraryError> {
        self.connection.execute(
            r#"
            INSERT INTO tracks (id, title, artist, album, discogs_release_id, discogs_confidence)
            VALUES (:id, :title, :artist, :album, :discogs_release_id, :discogs_confidence)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                discogs_release_id = excluded.discogs_release_id,
                discogs_confidence = excluded.discogs_confidence,
                updated_at = datetime('now');
            "#,
            rusqlite::named_params! {
                ":id": record.track_id,
                ":title": record.title,
                ":artist": record.artist,
                ":album": record.album,
                ":discogs_release_id": record.discogs_release_id,
                ":discogs_confidence": record.discogs_confidence.map(|value| value as f64),
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

    pub fn record_discogs_match(
        &self,
        record: &DiscogsMatchRecord,
        candidates: &[DiscogsCandidateRecord],
    ) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;
        self.persist_discogs_match(&transaction, record, candidates)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_discogs_candidates(
        &self,
        track_id: &str,
    ) -> Result<Vec<DiscogsCandidateRecord>, LibraryError> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT match_id, release_id, score, raw_payload
            FROM discogs_candidates
            WHERE match_id = :match_id
            ORDER BY score DESC;
            "#,
        )?;

        let mut rows = statement.query(rusqlite::named_params! { ":match_id": track_id })?;
        let mut result = Vec::new();

        while let Some(row) = rows.next()? {
            let match_id: String = row.get(0)?;
            let release_id: Option<String> = row.get(1)?;
            let score: Option<f64> = row.get(2)?;
            let raw_payload: String = row.get(3)?;
            let raw_payload: Value = serde_json::from_str(&raw_payload)?;

            result.push(DiscogsCandidateRecord {
                match_id,
                release_id,
                score: score.map(|value| value as f32),
                raw_payload,
            });
        }

        Ok(result)
    }

    fn persist_discogs_match(
        &self,
        transaction: &rusqlite::Transaction<'_>,
        record: &DiscogsMatchRecord,
        candidates: &[DiscogsCandidateRecord],
    ) -> Result<(), LibraryError> {
        transaction.execute(
            "INSERT OR IGNORE INTO tracks (id) VALUES (:track_id);",
            rusqlite::named_params! { ":track_id": &record.track_id },
        )?;

        transaction.execute(
            r#"
            INSERT INTO discogs_matches (track_id, release_id, confidence, status, query, message, checked_at)
            VALUES (:track_id, :release_id, :confidence, :status, :query, :message, COALESCE(:checked_at, datetime('now')))
            ON CONFLICT(track_id) DO UPDATE SET
                release_id = excluded.release_id,
                confidence = excluded.confidence,
                status = excluded.status,
                query = excluded.query,
                message = excluded.message,
                checked_at = excluded.checked_at;
            "#,
            rusqlite::named_params! {
                ":track_id": &record.track_id,
                ":release_id": record.release_id.as_ref(),
                ":confidence": record.confidence.map(|value| value as f64),
                ":status": record.status.as_str(),
                ":query": record.query.as_ref(),
                ":message": record.message.as_ref(),
                ":checked_at": record.checked_at.as_deref(),
            },
        )?;

        transaction.execute(
            r#"
            UPDATE tracks
            SET discogs_release_id = :release_id,
                discogs_confidence = :confidence,
                updated_at = datetime('now')
            WHERE id = :track_id;
            "#,
            rusqlite::named_params! {
                ":track_id": &record.track_id,
                ":release_id": record.release_id.as_ref(),
                ":confidence": record.confidence.map(|value| value as f64),
            },
        )?;

        transaction.execute(
            "DELETE FROM discogs_candidates WHERE match_id = :match_id;",
            rusqlite::named_params! { ":match_id": &record.track_id },
        )?;

        for candidate in candidates {
            if candidate.match_id != record.track_id {
                continue;
            }

            let raw_payload = serde_json::to_string(&candidate.raw_payload)?;
            transaction.execute(
                r#"
                INSERT INTO discogs_candidates (match_id, release_id, score, raw_payload)
                VALUES (:match_id, :release_id, :score, :raw_payload);
                "#,
                rusqlite::named_params! {
                    ":match_id": &record.track_id,
                    ":release_id": candidate.release_id.as_ref(),
                    ":score": candidate.score.map(|value| value as f64),
                    ":raw_payload": raw_payload,
                },
            )?;
        }

        Ok(())
    }

    pub fn record_discogs_success(
        &self,
        track_id: &str,
        query: &str,
        release: &Value,
        confidence: f32,
    ) -> Result<(), LibraryError> {
        let release_id = extract_release_id(release);
        let score = release
            .get("score")
            .and_then(|value| value.as_f64())
            .map(|value| value as f32)
            .or(Some(confidence));

        let candidate = DiscogsCandidateRecord {
            match_id: track_id.to_string(),
            release_id: release_id.clone(),
            score,
            raw_payload: release.clone(),
        };
        let record = DiscogsMatchRecord {
            track_id: track_id.to_string(),
            release_id,
            confidence: Some(confidence),
            status: DiscogsMatchStatus::Success,
            query: Some(query.to_string()),
            message: None,
            checked_at: None,
        };

        self.record_discogs_match(&record, &[candidate])
    }

    pub fn record_discogs_ambiguity(
        &self,
        track_id: &str,
        query: &str,
        candidates: &[Value],
    ) -> Result<(), LibraryError> {
        let candidate_records = candidates
            .iter()
            .filter_map(|candidate| {
                let release_id = extract_release_id(candidate);
                let raw_payload = candidate.clone();
                release_id.map(|release_id| DiscogsCandidateRecord {
                    match_id: track_id.to_string(),
                    release_id: Some(release_id),
                    score: candidate
                        .get("score")
                        .and_then(|value| value.as_f64())
                        .map(|value| value as f32),
                    raw_payload,
                })
            })
            .collect::<Vec<_>>();

        let record = DiscogsMatchRecord {
            track_id: track_id.to_string(),
            release_id: None,
            confidence: None,
            status: DiscogsMatchStatus::Ambiguous,
            query: Some(query.to_string()),
            message: None,
            checked_at: None,
        };

        self.record_discogs_match(&record, &candidate_records)
    }

    pub fn record_discogs_failure(
        &self,
        track_id: &str,
        query: &str,
        reason: &str,
    ) -> Result<(), LibraryError> {
        let record = DiscogsMatchRecord {
            track_id: track_id.to_string(),
            release_id: None,
            confidence: None,
            status: DiscogsMatchStatus::Error,
            query: Some(query.to_string()),
            message: Some(reason.to_string()),
            checked_at: None,
        };

        self.record_discogs_match(&record, &[])
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

    fn migrate_discogs_payloads(&self) -> Result<(), LibraryError> {
        let mut transaction = self.connection.transaction()?;

        {
            let mut statement = transaction.prepare(
                "SELECT id, discogs_payload FROM tracks WHERE discogs_payload IS NOT NULL;",
            )?;
            let mut rows = statement.query([])?;

            while let Some(row) = rows.next()? {
                let track_id: String = row.get(0)?;
                let payload_json: String = row.get(1)?;
                let payload: Value = serde_json::from_str(&payload_json)?;

                let status = payload
                    .get("status")
                    .and_then(|value| value.as_str())
                    .map(DiscogsMatchStatus::from_str)
                    .unwrap_or(DiscogsMatchStatus::Error);
                let query = payload
                    .get("query")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string());
                let message = payload
                    .get("reason")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string());

                let mut release_id = None;
                let mut confidence = payload
                    .get("confidence")
                    .and_then(|value| value.as_f64())
                    .map(|value| value as f32);
                let mut candidate_records = Vec::new();

                match status {
                    DiscogsMatchStatus::Success => {
                        if let Some(release) = payload.get("release") {
                            release_id = extract_release_id(release);
                            let score = release
                                .get("score")
                                .and_then(|value| value.as_f64())
                                .map(|value| value as f32)
                                .or(confidence);
                            candidate_records.push(DiscogsCandidateRecord {
                                match_id: track_id.clone(),
                                release_id: release_id.clone(),
                                score,
                                raw_payload: release.clone(),
                            });
                        }
                    }
                    DiscogsMatchStatus::Ambiguous => {
                        confidence = None;
                        if let Some(candidates) =
                            payload.get("candidates").and_then(|value| value.as_array())
                        {
                            for candidate in candidates {
                                if let Some(id) = extract_release_id(candidate) {
                                    candidate_records.push(DiscogsCandidateRecord {
                                        match_id: track_id.clone(),
                                        release_id: Some(id),
                                        score: candidate
                                            .get("score")
                                            .and_then(|value| value.as_f64())
                                            .map(|value| value as f32),
                                        raw_payload: candidate.clone(),
                                    });
                                }
                            }
                        }
                    }
                    DiscogsMatchStatus::Error => {
                        confidence = None;
                    }
                }

                let match_record = DiscogsMatchRecord {
                    track_id: track_id.clone(),
                    release_id,
                    confidence,
                    status,
                    query,
                    message,
                    checked_at: None,
                };

                self.persist_discogs_match(&transaction, &match_record, &candidate_records)?;
                transaction.execute(
                    "UPDATE tracks SET discogs_payload = NULL WHERE id = :track_id;",
                    rusqlite::named_params! { ":track_id": &track_id },
                )?;
            }
        }

        transaction.commit()?;
        Ok(())
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

fn extract_release_id(value: &Value) -> Option<String> {
    let id_value = value
        .get("id")
        .or_else(|| value.get("release_id"))
        .or_else(|| value.get("master_id"));

    match id_value {
        Some(Value::String(id)) => Some(id.clone()),
        Some(Value::Number(number)) => number
            .as_u64()
            .map(|value| value.to_string())
            .or_else(|| number.as_i64().map(|value| value.to_string())),
        _ => None,
    }
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

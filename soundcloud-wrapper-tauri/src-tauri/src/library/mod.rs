use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::rekordbox::RekordboxTrack;
use rusqlite::{params, Connection, ErrorCode};
use serde::Deserialize;
use serde::Serialize;
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
    #[serde(default)]
    pub musicbrainz_release_id: Option<String>,
    #[serde(default)]
    pub musicbrainz_confidence: Option<f32>,
    #[serde(default)]
    pub musicbrainz_payload: Option<Value>,
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

#[derive(Debug, Clone, Copy)]
pub enum MusicbrainzMatchStatus {
    Success,
    Ambiguous,
    Error,
}

impl MusicbrainzMatchStatus {
    fn as_str(&self) -> &'static str {
        match self {
            MusicbrainzMatchStatus::Success => "success",
            MusicbrainzMatchStatus::Ambiguous => "ambiguous",
            MusicbrainzMatchStatus::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MusicbrainzMatchRecord {
    pub track_id: String,
    pub release_id: Option<String>,
    pub confidence: Option<f32>,
    pub status: MusicbrainzMatchStatus,
    pub query: Option<String>,
    pub message: Option<String>,
    pub checked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MusicbrainzCandidateRecord {
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

#[derive(Debug, Clone)]
pub struct SoundcloudLookupRecord {
    pub track_id: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub soundcloud_id: Option<String>,
    pub permalink_url: Option<String>,
    pub raw_payload: Option<Value>,
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

/// Describes a single row returned by [`LibraryStore::list_library_status`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatusRow {
    pub track_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    pub liked: bool,
    pub matched: bool,
    pub has_local_file: bool,
    pub local_available: bool,
    pub in_rekordbox: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discogs_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discogs_release_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discogs_confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discogs_checked_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discogs_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz_release_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz_confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz_checked_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soundcloud_permalink_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soundcloud_liked_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_location: Option<String>,
}

/// A paginated response produced by [`LibraryStore::list_library_status`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatusPage {
    pub rows: Vec<LibraryStatusRow>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

/// Filtering and pagination options for [`LibraryStore::list_library_status`].
///
/// * `missing_assets_only` &mdash; return tracks that do not have an available
///   local asset entry. This includes tracks that have never been downloaded or
///   where the asset is marked as unavailable.
/// * `unresolved_discogs_only` &mdash; return tracks where the Discogs
///   integration has not produced a successful match.
/// * `liked_only` &mdash; limit results to tracks that have a SoundCloud payload
///   containing a `likedAt` timestamp.
/// * `rekordbox_only` &mdash; limit results to tracks that currently have a
///   Rekordbox source entry.
/// * `limit` / `offset` &mdash; standard pagination controls applied to the
///   ordered result set. The backend enforces sensible defaults to avoid
///   fetching excessively large pages.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusFilter {
    pub missing_assets_only: bool,
    pub unresolved_discogs_only: bool,
    pub liked_only: bool,
    pub rekordbox_only: bool,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
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
                musicbrainz_payload TEXT,
                musicbrainz_release_id TEXT,
                musicbrainz_confidence REAL,
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

            CREATE TABLE IF NOT EXISTS musicbrainz_matches (
                track_id TEXT PRIMARY KEY,
                release_id TEXT,
                confidence REAL,
                status TEXT NOT NULL,
                query TEXT,
                message TEXT,
                checked_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS musicbrainz_candidates (
                match_id TEXT NOT NULL,
                release_id TEXT,
                score REAL,
                raw_payload TEXT NOT NULL,
                FOREIGN KEY(match_id) REFERENCES musicbrainz_matches(track_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS musicbrainz_matches_release_idx ON musicbrainz_matches(release_id);
            CREATE INDEX IF NOT EXISTS musicbrainz_matches_status_idx ON musicbrainz_matches(status);
            CREATE INDEX IF NOT EXISTS musicbrainz_candidates_match_idx ON musicbrainz_candidates(match_id);
            CREATE INDEX IF NOT EXISTS musicbrainz_candidates_release_idx ON musicbrainz_candidates(release_id);
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

        if let Err(error) = self.connection.execute(
            "ALTER TABLE tracks ADD COLUMN musicbrainz_payload TEXT;",
            [],
        ) {
            match error {
                rusqlite::Error::SqliteFailure(ref failure, _)
                    if failure.code == ErrorCode::DuplicateColumnName => {}
                _ => return Err(error.into()),
            }
        }

        if let Err(error) = self.connection.execute(
            "ALTER TABLE tracks ADD COLUMN musicbrainz_release_id TEXT;",
            [],
        ) {
            match error {
                rusqlite::Error::SqliteFailure(ref failure, _)
                    if failure.code == ErrorCode::DuplicateColumnName => {}
                _ => return Err(error.into()),
            }
        }

        if let Err(error) = self.connection.execute(
            "ALTER TABLE tracks ADD COLUMN musicbrainz_confidence REAL;",
            [],
        ) {
            match error {
                rusqlite::Error::SqliteFailure(ref failure, _)
                    if failure.code == ErrorCode::DuplicateColumnName => {}
                _ => return Err(error.into()),
            }
        }

        self.migrate_discogs_payloads()?;
        self.migrate_musicbrainz_payloads()?;
        Ok(())
    }

    pub fn upsert_track(&self, record: &TrackRecord) -> Result<(), LibraryError> {
        let musicbrainz_payload = record
            .musicbrainz_payload
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.connection.execute(
            r#"
            INSERT INTO tracks (
                id,
                title,
                artist,
                album,
                discogs_release_id,
                discogs_confidence,
                musicbrainz_release_id,
                musicbrainz_confidence,
                musicbrainz_payload
            )
            VALUES (
                :id,
                :title,
                :artist,
                :album,
                :discogs_release_id,
                :discogs_confidence,
                :musicbrainz_release_id,
                :musicbrainz_confidence,
                :musicbrainz_payload
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                discogs_release_id = excluded.discogs_release_id,
                discogs_confidence = excluded.discogs_confidence,
                musicbrainz_release_id = excluded.musicbrainz_release_id,
                musicbrainz_confidence = excluded.musicbrainz_confidence,
                musicbrainz_payload = excluded.musicbrainz_payload,
                updated_at = datetime('now');
            "#,
            rusqlite::named_params! {
                ":id": record.track_id,
                ":title": record.title,
                ":artist": record.artist,
                ":album": record.album,
                ":discogs_release_id": record.discogs_release_id,
                ":discogs_confidence": record.discogs_confidence.map(|value| value as f64),
                ":musicbrainz_release_id": record.musicbrainz_release_id,
                ":musicbrainz_confidence": record.musicbrainz_confidence.map(|value| value as f64),
                ":musicbrainz_payload": musicbrainz_payload,
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

    pub fn record_musicbrainz_match(
        &self,
        record: &MusicbrainzMatchRecord,
        candidates: &[MusicbrainzCandidateRecord],
    ) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;
        self.persist_musicbrainz_match(&transaction, record, candidates)?;
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

    pub fn load_soundcloud_lookup(
        &self,
        track_id: &str,
    ) -> Result<Option<SoundcloudLookupRecord>, LibraryError> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT t.id, t.title, t.artist, ss.soundcloud_id, ss.permalink_url, ss.raw_payload
            FROM tracks t
            LEFT JOIN soundcloud_sources ss ON ss.track_id = t.id
            WHERE t.id = :track_id;
            "#,
        )?;

        let mut rows = statement.query(rusqlite::named_params! { ":track_id": track_id })?;
        if let Some(row) = rows.next()? {
            let raw_payload: Option<String> = row.get(5)?;
            let raw_payload = match raw_payload {
                Some(payload) => Some(serde_json::from_str(&payload)?),
                None => None,
            };

            Ok(Some(SoundcloudLookupRecord {
                track_id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                soundcloud_id: row.get(3)?,
                permalink_url: row.get(4)?,
                raw_payload,
            }))
        } else {
            Ok(None)
        }
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

    fn persist_musicbrainz_match(
        &self,
        transaction: &rusqlite::Transaction<'_>,
        record: &MusicbrainzMatchRecord,
        candidates: &[MusicbrainzCandidateRecord],
    ) -> Result<(), LibraryError> {
        transaction.execute(
            "INSERT OR IGNORE INTO tracks (id) VALUES (:track_id);",
            rusqlite::named_params! { ":track_id": &record.track_id },
        )?;

        transaction.execute(
            r#"
            INSERT INTO musicbrainz_matches (track_id, release_id, confidence, status, query, message, checked_at)
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
            SET musicbrainz_release_id = :release_id,
                musicbrainz_confidence = :confidence,
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
            "DELETE FROM musicbrainz_candidates WHERE match_id = :match_id;",
            rusqlite::named_params! { ":match_id": &record.track_id },
        )?;

        for candidate in candidates {
            let raw_payload = serde_json::to_string(&candidate.raw_payload)?;
            transaction.execute(
                r#"
                INSERT INTO musicbrainz_candidates (match_id, release_id, score, raw_payload)
                VALUES (:match_id, :release_id, :score, :raw_payload);
                "#,
                rusqlite::named_params! {
                    ":match_id": &candidate.match_id,
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

    pub fn record_musicbrainz_success(
        &self,
        track_id: &str,
        query: &str,
        release: &Value,
        confidence: f32,
    ) -> Result<(), LibraryError> {
        let release_id = extract_release_id(release);
        let candidate = MusicbrainzCandidateRecord {
            match_id: track_id.to_string(),
            release_id: release_id.clone(),
            score: Some(confidence),
            raw_payload: release.clone(),
        };
        let record = MusicbrainzMatchRecord {
            track_id: track_id.to_string(),
            release_id,
            confidence: Some(confidence),
            status: MusicbrainzMatchStatus::Success,
            query: Some(query.to_string()),
            message: None,
            checked_at: None,
        };

        self.record_musicbrainz_match(&record, &[candidate])
    }

    pub fn record_musicbrainz_ambiguity(
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
                release_id.map(|release_id| MusicbrainzCandidateRecord {
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

        let record = MusicbrainzMatchRecord {
            track_id: track_id.to_string(),
            release_id: None,
            confidence: None,
            status: MusicbrainzMatchStatus::Ambiguous,
            query: Some(query.to_string()),
            message: None,
            checked_at: None,
        };

        self.record_musicbrainz_match(&record, &candidate_records)
    }

    pub fn record_musicbrainz_failure(
        &self,
        track_id: &str,
        query: &str,
        reason: &str,
    ) -> Result<(), LibraryError> {
        let record = MusicbrainzMatchRecord {
            track_id: track_id.to_string(),
            release_id: None,
            confidence: None,
            status: MusicbrainzMatchStatus::Error,
            query: Some(query.to_string()),
            message: Some(reason.to_string()),
            checked_at: None,
        };

        self.record_musicbrainz_match(&record, &[])
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

    pub fn list_library_status(
        &self,
        filter: &StatusFilter,
    ) -> Result<LibraryStatusPage, LibraryError> {
        const DEFAULT_LIMIT: u32 = 100;
        const MAX_LIMIT: u32 = 500;

        let requested_limit = filter.limit.unwrap_or(DEFAULT_LIMIT);
        let limit = requested_limit.max(1).min(MAX_LIMIT) as i64;
        let offset_value = filter.offset.unwrap_or(0) as i64;

        let liked_predicate = "json_extract(ss.raw_payload, '$.likedAt') IS NOT NULL";

        let mut conditions: Vec<&'static str> = Vec::new();
        if filter.missing_assets_only {
            conditions.push("(la.track_id IS NULL OR la.available = 0)");
        }
        if filter.unresolved_discogs_only {
            conditions
                .push("(dm.track_id IS NULL OR dm.status != 'success' OR dm.release_id IS NULL)");
        }
        if filter.liked_only {
            conditions.push(liked_predicate);
        }
        if filter.rekordbox_only {
            conditions.push("rb.track_id IS NOT NULL");
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let from_clause = r#"
            FROM tracks t
            LEFT JOIN soundcloud_sources ss ON ss.track_id = t.id
            LEFT JOIN discogs_matches dm ON dm.track_id = t.id
            LEFT JOIN musicbrainz_matches mb ON mb.track_id = t.id
            LEFT JOIN local_assets la ON la.track_id = t.id
            LEFT JOIN rekordbox_sources rb ON rb.track_id = t.id
        "#;

        let count_query = format!("SELECT COUNT(*) {from_clause} {where_clause};");
        let mut count_statement = self.connection.prepare(&count_query)?;
        let total: i64 = count_statement.query_row([], |row| row.get(0))?;

        let select_query = format!(
            r#"
            SELECT
                t.id,
                t.title,
                t.artist,
                t.album,
                CASE WHEN {liked_predicate} THEN 1 ELSE 0 END AS liked,
                CASE WHEN dm.status = 'success' AND dm.release_id IS NOT NULL THEN 1 ELSE 0 END AS matched,
                CASE WHEN la.track_id IS NOT NULL THEN 1 ELSE 0 END AS has_local,
                CASE WHEN la.track_id IS NOT NULL AND la.available = 1 THEN 1 ELSE 0 END AS local_available,
                CASE WHEN rb.track_id IS NOT NULL THEN 1 ELSE 0 END AS in_rekordbox,
                dm.status,
                dm.release_id,
                dm.confidence,
                dm.checked_at,
                dm.message,
                mb.status,
                mb.release_id,
                mb.confidence,
                mb.checked_at,
                mb.message,
                ss.permalink_url,
                json_extract(ss.raw_payload, '$.likedAt') AS liked_at,
                la.location
            {from_clause}
            {where_clause}
            ORDER BY t.updated_at DESC, t.id ASC
            LIMIT :limit OFFSET :offset;
            "#
        );

        let mut statement = self.connection.prepare(&select_query)?;
        let mut rows = statement.query(rusqlite::named_params! {
            ":limit": limit,
            ":offset": offset_value,
        })?;

        let mut result_rows = Vec::new();
        while let Some(row) = rows.next()? {
            let confidence: Option<f64> = row.get(11)?;
            let musicbrainz_confidence: Option<f64> = row.get(16)?;

            result_rows.push(LibraryStatusRow {
                track_id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                liked: row.get::<_, i64>(4)? != 0,
                matched: row.get::<_, i64>(5)? != 0,
                has_local_file: row.get::<_, i64>(6)? != 0,
                local_available: row.get::<_, i64>(7)? != 0,
                in_rekordbox: row.get::<_, i64>(8)? != 0,
                discogs_status: row.get(9)?,
                discogs_release_id: row.get(10)?,
                discogs_confidence: confidence.map(|value| value as f32),
                discogs_checked_at: row.get(12)?,
                discogs_message: row.get(13)?,
                musicbrainz_status: row.get(14)?,
                musicbrainz_release_id: row.get(15)?,
                musicbrainz_confidence: musicbrainz_confidence.map(|value| value as f32),
                musicbrainz_checked_at: row.get(17)?,
                musicbrainz_message: row.get(18)?,
                soundcloud_permalink_url: row.get(19)?,
                soundcloud_liked_at: row.get(20)?,
                local_location: row.get(21)?,
            });
        }

        let total = if total <= 0 { 0 } else { total as u32 };

        Ok(LibraryStatusPage {
            rows: result_rows,
            total,
            limit: limit as u32,
            offset: offset_value as u32,
        })
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

    fn migrate_musicbrainz_payloads(&self) -> Result<(), LibraryError> {
        let mut transaction = self.connection.transaction()?;

        {
            let mut statement = transaction.prepare(
                "SELECT id, musicbrainz_payload FROM tracks WHERE musicbrainz_payload IS NOT NULL;",
            )?;
            let mut rows = statement.query([])?;

            while let Some(row) = rows.next()? {
                let track_id: String = row.get(0)?;
                let payload_json: String = row.get(1)?;
                let payload: Value = serde_json::from_str(&payload_json)?;

                let status = payload
                    .get("status")
                    .and_then(|value| value.as_str())
                    .unwrap_or("error")
                    .to_string();
                let query = payload
                    .get("query")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string());
                let message = payload
                    .get("reason")
                    .or_else(|| payload.get("message"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string());

                let mut release_id = payload
                    .get("release_id")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string());
                let mut confidence = payload
                    .get("confidence")
                    .and_then(|value| value.as_f64())
                    .map(|value| value as f32);
                let mut candidate_payloads: Vec<(Option<String>, Option<f64>, Value)> = Vec::new();

                match status.as_str() {
                    "success" => {
                        if let Some(release) = payload
                            .get("release")
                            .or_else(|| payload.get("recording"))
                            .or_else(|| payload.get("match"))
                        {
                            if let Some(score_value) =
                                release.get("score").and_then(|value| value.as_f64())
                            {
                                confidence = Some(score_value as f32);
                            }

                            let extracted_id = extract_release_id(release);
                            if release_id.is_none() {
                                release_id = extracted_id.clone();
                            }

                            let candidate_score = release
                                .get("score")
                                .and_then(|value| value.as_f64())
                                .or_else(|| confidence.as_ref().map(|value| f64::from(*value)));
                            candidate_payloads.push((
                                extracted_id,
                                candidate_score,
                                release.clone(),
                            ));
                        }
                    }
                    "ambiguous" => {
                        confidence = None;
                        if let Some(candidates) =
                            payload.get("candidates").and_then(|value| value.as_array())
                        {
                            for candidate in candidates {
                                let candidate_id = extract_release_id(candidate);
                                let candidate_score =
                                    candidate.get("score").and_then(|value| value.as_f64());
                                candidate_payloads.push((
                                    candidate_id,
                                    candidate_score,
                                    candidate.clone(),
                                ));
                            }
                        }
                    }
                    _ => {
                        confidence = None;
                    }
                }

                let confidence_value = confidence.map(|value| value as f64);

                transaction.execute(
                    r#"
                    INSERT INTO musicbrainz_matches (track_id, release_id, confidence, status, query, message, checked_at)
                    VALUES (:track_id, :release_id, :confidence, :status, :query, :message, datetime('now'))
                    ON CONFLICT(track_id) DO UPDATE SET
                        release_id = excluded.release_id,
                        confidence = excluded.confidence,
                        status = excluded.status,
                        query = excluded.query,
                        message = excluded.message,
                        checked_at = excluded.checked_at;
                    "#,
                    rusqlite::named_params! {
                        ":track_id": &track_id,
                        ":release_id": release_id.as_ref(),
                        ":confidence": confidence_value,
                        ":status": &status,
                        ":query": query.as_ref(),
                        ":message": message.as_ref(),
                    },
                )?;

                transaction.execute(
                    r#"
                    UPDATE tracks
                    SET musicbrainz_release_id = :release_id,
                        musicbrainz_confidence = :confidence,
                        musicbrainz_payload = NULL,
                        updated_at = datetime('now')
                    WHERE id = :track_id;
                    "#,
                    rusqlite::named_params! {
                        ":track_id": &track_id,
                        ":release_id": release_id.as_ref(),
                        ":confidence": confidence_value,
                    },
                )?;

                transaction.execute(
                    "DELETE FROM musicbrainz_candidates WHERE match_id = :match_id;",
                    rusqlite::named_params! { ":match_id": &track_id },
                )?;

                for (candidate_id, candidate_score, candidate_payload) in candidate_payloads {
                    let raw_payload = serde_json::to_string(&candidate_payload)?;
                    transaction.execute(
                        r#"
                        INSERT INTO musicbrainz_candidates (match_id, release_id, score, raw_payload)
                        VALUES (:match_id, :release_id, :score, :raw_payload);
                        "#,
                        rusqlite::named_params! {
                            ":match_id": &track_id,
                            ":release_id": candidate_id.as_ref(),
                            ":score": candidate_score,
                            ":raw_payload": raw_payload,
                        },
                    )?;
                }
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

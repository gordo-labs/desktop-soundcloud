use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use quick_xml::de::from_reader as from_xml_reader;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone, Serialize)]
pub struct RekordboxCue {
    pub slot: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub position_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cue_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RekordboxTrack {
    pub rekordbox_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub available: bool,
    pub cues: Vec<RekordboxCue>,
}

#[derive(Debug)]
pub enum RekordboxError {
    Io(std::io::Error),
    Database(rusqlite::Error),
    Xml(quick_xml::DeError),
    Audio(SymphoniaError),
}

impl fmt::Display for RekordboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RekordboxError::Io(error) => write!(f, "filesystem error: {error}"),
            RekordboxError::Database(error) => write!(f, "sqlite error: {error}"),
            RekordboxError::Xml(error) => write!(f, "xml error: {error}"),
            RekordboxError::Audio(error) => write!(f, "audio probe error: {error}"),
        }
    }
}

impl std::error::Error for RekordboxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RekordboxError::Io(error) => Some(error),
            RekordboxError::Database(error) => Some(error),
            RekordboxError::Xml(error) => Some(error),
            RekordboxError::Audio(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for RekordboxError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for RekordboxError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<quick_xml::DeError> for RekordboxError {
    fn from(value: quick_xml::DeError) -> Self {
        Self::Xml(value)
    }
}

impl From<SymphoniaError> for RekordboxError {
    fn from(value: SymphoniaError) -> Self {
        Self::Audio(value)
    }
}

pub fn load_tracks(path: &Path) -> Result<Vec<RekordboxTrack>, RekordboxError> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("xml"))
    {
        Some(true) => parse_xml_export(path),
        _ => parse_master_db(path),
    }
}

pub fn supports_auto_refresh(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("xml") => false,
        _ => true,
    }
}

fn parse_master_db(path: &Path) -> Result<Vec<RekordboxTrack>, RekordboxError> {
    let connection = Connection::open(path)?;
    let mut cue_statement =
        connection.prepare("SELECT SongID, HotCueNo, InMsec, Name, Color, Type FROM djmdHotCue")?;

    let mut cue_rows = cue_statement.query([])?;
    let mut cue_map: HashMap<i64, Vec<RekordboxCue>> = HashMap::new();

    while let Some(row) = cue_rows.next()? {
        let song_id: i64 = row.get(0)?;
        let slot: i64 = row.get(1)?;
        let position: i64 = row.get::<_, Option<i64>>(2)?.unwrap_or_default();
        let name: Option<String> = row.get(3)?;
        let color: Option<String> = row.get(4)?;
        let cue_type: Option<String> = row.get(5)?;

        cue_map.entry(song_id).or_default().push(RekordboxCue {
            slot,
            name,
            color,
            position_ms: position,
            cue_type,
        });
    }

    let mut statement = connection.prepare(
        "SELECT ID, TrackID, Title, Artist, Album, FilePath, FolderPath, FileName FROM djmdSong",
    )?;

    let mut rows = statement.query([])?;
    let mut tracks = Vec::new();

    while let Some(row) = rows.next()? {
        let rekordbox_id: i64 = row.get(0)?;
        let rekordbox_id_str = rekordbox_id.to_string();
        let track_reference: Option<String> = row.get(1)?;
        let title: Option<String> = row.get(2)?;
        let artist: Option<String> = row.get(3)?;
        let album: Option<String> = row.get(4)?;
        let file_path_value: Option<String> = row.get(5)?;
        let folder_path: Option<String> = row.get(6)?;
        let file_name: Option<String> = row.get(7)?;

        let location = resolve_location(&file_path_value, &folder_path, &file_name);
        let normalized_path = location.as_ref().and_then(|value| decode_location(value));

        let metadata = normalized_path
            .as_ref()
            .and_then(|path| match compute_file_metadata(path) {
                Ok(metadata) => Some(metadata),
                Err(error) => {
                    eprintln!(
                        "failed to compute metadata for rekordbox entry {rekordbox_id_str}: {error}"
                    );
                    None
                }
            })
            .unwrap_or_else(FileMetadata::missing);

        tracks.push(RekordboxTrack {
            rekordbox_id: rekordbox_id_str,
            track_reference,
            title,
            artist,
            album,
            location,
            normalized_path,
            checksum: metadata.checksum,
            duration_ms: metadata.duration_ms,
            available: metadata.available,
            cues: cue_map.remove(&rekordbox_id).unwrap_or_default(),
        });
    }

    Ok(tracks)
}

#[derive(Debug, Deserialize)]
struct XmlRoot {
    #[serde(rename = "COLLECTION")]
    collection: Option<XmlCollection>,
}

#[derive(Debug, Deserialize)]
struct XmlCollection {
    #[serde(rename = "TRACK", default)]
    tracks: Vec<XmlTrack>,
}

#[derive(Debug, Deserialize)]
struct XmlTrack {
    #[serde(rename = "@TrackID")]
    track_id: Option<String>,
    #[serde(rename = "@Name")]
    name: Option<String>,
    #[serde(rename = "@Artist")]
    artist: Option<String>,
    #[serde(rename = "@Album")]
    album: Option<String>,
    #[serde(rename = "@Location")]
    location: Option<String>,
    #[serde(rename = "@RekordboxID")]
    rekordbox_id: Option<String>,
    #[serde(rename = "POSITION_MARK", default)]
    position_marks: Vec<XmlCue>,
}

#[derive(Debug, Deserialize)]
struct XmlCue {
    #[serde(rename = "@Num")]
    slot: Option<i64>,
    #[serde(rename = "@Name")]
    name: Option<String>,
    #[serde(rename = "@Color")]
    color: Option<String>,
    #[serde(rename = "@Type")]
    cue_type: Option<String>,
    #[serde(rename = "@Start")]
    start: Option<f64>,
}

fn parse_xml_export(path: &Path) -> Result<Vec<RekordboxTrack>, RekordboxError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let root: XmlRoot = from_xml_reader(reader)?;
    let collection = match root.collection {
        Some(collection) => collection,
        None => return Ok(Vec::new()),
    };

    let mut result = Vec::new();

    for entry in collection.tracks {
        let rekordbox_id = match entry
            .rekordbox_id
            .clone()
            .or_else(|| entry.track_id.clone())
        {
            Some(id) => id,
            None => {
                eprintln!(
                    "skipping rekordbox XML track without an identifier: {:?}",
                    entry.name
                );
                continue;
            }
        };

        let normalized_path = entry
            .location
            .as_ref()
            .and_then(|value| decode_location(value));

        let metadata = normalized_path
            .as_ref()
            .and_then(|path| match compute_file_metadata(path) {
                Ok(metadata) => Some(metadata),
                Err(error) => {
                    eprintln!(
                        "failed to compute metadata for rekordbox entry {rekordbox_id}: {error}"
                    );
                    None
                }
            })
            .unwrap_or_else(FileMetadata::missing);

        let cues = entry
            .position_marks
            .into_iter()
            .map(|cue| RekordboxCue {
                slot: cue.slot.unwrap_or_default(),
                name: cue.name,
                color: cue.color,
                position_ms: cue
                    .start
                    .map(|value| (value * 1000.0) as i64)
                    .unwrap_or_default(),
                cue_type: cue.cue_type,
            })
            .collect();

        result.push(RekordboxTrack {
            rekordbox_id,
            track_reference: entry.track_id,
            title: entry.name,
            artist: entry.artist,
            album: entry.album,
            location: entry.location.clone(),
            normalized_path,
            checksum: metadata.checksum,
            duration_ms: metadata.duration_ms,
            available: metadata.available,
            cues,
        });
    }

    Ok(result)
}

fn resolve_location(
    file_path: &Option<String>,
    folder_path: &Option<String>,
    file_name: &Option<String>,
) -> Option<String> {
    if let Some(path) = file_path.clone() {
        if !path.is_empty() {
            return Some(path);
        }
    }

    match (folder_path, file_name) {
        (Some(folder), Some(name)) => {
            if folder.ends_with('/') {
                Some(format!("{folder}{name}"))
            } else {
                Some(format!("{folder}/{name}"))
            }
        }
        _ => None,
    }
}

fn decode_location(value: &str) -> Option<PathBuf> {
    if value.starts_with("file://") {
        if let Ok(url) = url::Url::parse(value) {
            if url.scheme() == "file" {
                return url.to_file_path().ok();
            }
        }
    }

    Some(PathBuf::from(value))
}

struct FileMetadata {
    checksum: Option<String>,
    duration_ms: Option<u64>,
    available: bool,
}

impl FileMetadata {
    fn missing() -> Self {
        Self {
            checksum: None,
            duration_ms: None,
            available: false,
        }
    }
}

fn compute_file_metadata(path: &Path) -> Result<FileMetadata, RekordboxError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileMetadata::missing());
        }
        Err(error) => return Err(error.into()),
    };

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes = reader.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        hasher.update(&buffer[..bytes]);
    }

    let checksum = format!("{:x}", hasher.finalize());

    let duration_ms = compute_duration(path).unwrap_or(None);

    Ok(FileMetadata {
        checksum: Some(checksum),
        duration_ms,
        available: true,
    })
}

fn compute_duration(path: &Path) -> Result<Option<u64>, RekordboxError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(extension);
    }

    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let probed =
        symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| SymphoniaError::ResetRequired)?;

    let decoder_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &decoder_opts)?;
    let mut duration = 0u64;
    let mut sample_rate = track.codec_params.sample_rate;

    loop {
        match format.next_packet() {
            Ok(packet) => {
                let decoded = decoder.decode(&packet)?;
                if sample_rate.is_none() {
                    sample_rate = Some(decoded.spec().rate);
                }
                let frames = decoded.frames();
                duration += frames as u64;
            }
            Err(SymphoniaError::IoError(ref error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                break;
            }
            Err(err) => return Err(RekordboxError::Audio(err)),
        }
    }

    let sample_rate = match sample_rate {
        Some(rate) if rate > 0 => rate,
        _ => return Ok(None),
    };

    if sample_rate == 0 {
        return Ok(None);
    }

    let seconds = duration as f64 / sample_rate as f64;
    Ok(Some((seconds * 1000.0) as u64))
}

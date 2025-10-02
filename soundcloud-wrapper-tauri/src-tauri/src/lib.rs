mod discogs;
mod library;
mod media;
mod musicbrainz;
mod rekordbox;

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use discogs::DiscogsService;
use library::{
    DiscogsCandidateRecord, LibraryStatusPage, LibraryStore, LocalAssetRecord,
    MusicbrainzCandidateRecord, SoundcloudLookupRecord, SoundcloudSourceRecord, StatusFilter,
    TrackRecord,
};
use media::{MediaCache, MediaIntegration, MediaUpdate, MediaUpdatePayload, ThemeChangePayload};
use musicbrainz::MusicbrainzService;
use rekordbox::{load_tracks, supports_auto_refresh};
use serde::Deserialize;
use serde_json::{self, Value};
use tauri::async_runtime::{self, JoinHandle};
use tauri::menu::MenuBuilder;
use tauri::plugin::Builder as PluginBuilder;
use tauri::tray::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_shell::ShellExt;

const MAIN_WINDOW_LABEL: &str = "main";
const MEDIA_TOGGLE_EVENT: &str = "media://toggle";
const MEDIA_PLAY_EVENT: &str = "media://play";
const MEDIA_PAUSE_EVENT: &str = "media://pause";
const MEDIA_NEXT_EVENT: &str = "media://next";
const MEDIA_PREVIOUS_EVENT: &str = "media://previous";
const MEDIA_STATE_EVENT: &str = "app://media/state";
const THEME_CHANGE_EVENT: &str = "app://theme/change";
const TRAY_HOME_EVENT: &str = "app://tray/home";
const TRAY_MENU_TOGGLE: &str = "tray://toggle";
const TRAY_MENU_HOME: &str = "tray://home";
const TRAY_MENU_EXIT: &str = "tray://exit";
const LIBRARY_LIKE_EVENT: &str = "app://library/like-updated";
const LIBRARY_PLAYLIST_EVENT: &str = "app://library/playlist-updated";
const LIBRARY_REFRESH_LIKES_EVENT: &str = "app://library/likes/refresh";

struct AppState {
    media: Mutex<MediaManager>,
    library: Arc<Mutex<LibraryStore>>,
    discogs: DiscogsService,
    musicbrainz: MusicbrainzService,
    rekordbox: Mutex<RekordboxState>,
}

struct MediaManager {
    integration: MediaIntegration,
    cache: MediaCache,
}

#[derive(Default)]
struct RekordboxState {
    watcher: Option<RekordboxWatcher>,
}

struct RekordboxWatcher {
    path: PathBuf,
    handle: JoinHandle<()>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SoundcloudTrackPayload {
    track_id: String,
    soundcloud_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    permalink_url: Option<String>,
    #[serde(default)]
    artwork_url: Option<String>,
    #[serde(default)]
    duration_ms: Option<i64>,
    #[serde(default)]
    liked_at: Option<String>,
    #[serde(default)]
    playlist_id: Option<String>,
    #[serde(default)]
    playlist_position: Option<i64>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    raw: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoundcloudPlaylistPayload {
    playlist_id: String,
    soundcloud_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    permalink_url: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    track_count: Option<u32>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    raw: Value,
    #[serde(default)]
    tracks: Vec<SoundcloudTrackPayload>,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self, library::LibraryError> {
        let library = LibraryStore::initialize(app)?;

        let library = Arc::new(Mutex::new(library));
        let discogs = DiscogsService::new(app, Arc::clone(&library));
        let musicbrainz = MusicbrainzService::new(app, Arc::clone(&library));

        Ok(Self {
            media: Mutex::new(MediaManager {
                integration: MediaIntegration::initialize(app),
                cache: MediaCache::default(),
            }),
            library,
            discogs,
            musicbrainz,
            rekordbox: Mutex::new(RekordboxState::default()),
        })
    }
}

impl RekordboxState {
    fn configure(&mut self, path: PathBuf, store: Arc<Mutex<LibraryStore>>) {
        if let Some(existing) = self.watcher.as_ref() {
            if existing.path == path {
                return;
            }
        }
        self.watcher = Some(RekordboxWatcher::spawn(path, store));
    }

    fn disable(&mut self) {
        self.watcher = None;
    }
}

impl RekordboxWatcher {
    fn spawn(path: PathBuf, store: Arc<Mutex<LibraryStore>>) -> Self {
        let watch_path = path.clone();
        let handle = async_runtime::spawn(async move {
            let mut last_modified = fs::metadata(&watch_path)
                .and_then(|meta| meta.modified())
                .ok();

            loop {
                async_runtime::sleep(Duration::from_secs(30)).await;

                let metadata = match fs::metadata(&watch_path) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        eprintln!("failed to read rekordbox database metadata: {error}");
                        continue;
                    }
                };

                let modified = match metadata.modified() {
                    Ok(modified) => modified,
                    Err(error) => {
                        eprintln!("failed to read rekordbox database modification time: {error}");
                        continue;
                    }
                };

                let changed = last_modified
                    .map(|previous| modified > previous)
                    .unwrap_or(true);

                if changed {
                    last_modified = Some(modified);
                    let import_path = watch_path.clone();
                    match async_runtime::spawn_blocking(move || load_tracks(&import_path)).await {
                        Ok(Ok(tracks)) => {
                            let mut guard = match store.lock() {
                                Ok(guard) => guard,
                                Err(_) => {
                                    eprintln!(
                                        "failed to acquire library store lock during rekordbox refresh"
                                    );
                                    continue;
                                }
                            };

                            if let Err(error) = guard.sync_rekordbox_tracks(&tracks) {
                                eprintln!("failed to persist rekordbox refresh: {error}");
                            }
                        }
                        Ok(Err(error)) => {
                            eprintln!("failed to refresh rekordbox library: {error}");
                        }
                        Err(error) => {
                            eprintln!("failed to join rekordbox refresh task: {error}");
                        }
                    }
                }
            }
        });

        Self { path, handle }
    }
}

impl Drop for RekordboxWatcher {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[derive(Default)]
struct WindowState {
    hidden: AtomicBool,
}

struct TrayState(TrayIcon);

#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|error| format!("invalid URL: {error}"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("URLs with embedded credentials are not allowed".into());
    }
    match parsed.scheme() {
        "http" => {
            if parsed.host_str() != Some("localhost") {
                return Err("http scheme is only allowed for the local development server".into());
            }
            let target = parsed.into_string();
            app.shell()
                .open(target, None)
                .map_err(|error| format!("failed to open URL externally: {error}"))
        }
        "https" => {
            let target = parsed.into_string();
            app.shell()
                .open(target, None)
                .map_err(|error| format!("failed to open URL externally: {error}"))
        }
        scheme => Err(format!("unsupported scheme '{scheme}'")),
    }
}

#[tauri::command]
fn refresh_soundcloud_likes(app: AppHandle) -> Result<(), String> {
    app.emit_to(MAIN_WINDOW_LABEL, LIBRARY_REFRESH_LIKES_EVENT, ())
        .map_err(|error| format!("failed to request SoundCloud likes refresh: {error}"))
}

#[tauri::command]
fn retry_discogs_lookup(state: tauri::State<AppState>, track_id: String) -> Result<(), String> {
    let payload = resolve_lookup_payload(&state, &track_id)?;
    state.discogs.queue_lookup(payload);
    Ok(())
}

#[tauri::command]
fn retry_musicbrainz_lookup(state: tauri::State<AppState>, track_id: String) -> Result<(), String> {
    let payload = resolve_lookup_payload(&state, &track_id)?;
    state.musicbrainz.queue_lookup(payload);
    Ok(())
}

#[tauri::command]
fn confirm_musicbrainz_match(
    state: tauri::State<AppState>,
    track_id: String,
    release: Value,
    confidence: Option<f32>,
    query: Option<String>,
) -> Result<(), String> {
    let query_value = query.unwrap_or_default();
    let resolved_confidence = confidence
        .or_else(|| {
            release
                .get("score")
                .and_then(|value| value.as_f64())
                .map(|value| value as f32)
        })
        .unwrap_or(100.0);

    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .record_musicbrainz_success(&track_id, &query_value, &release, resolved_confidence)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn upsert_track(state: tauri::State<AppState>, record: TrackRecord) -> Result<(), String> {
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .upsert_track(&record)
        .map_err(|error| error.to_string())
}

fn resolve_lookup_payload(
    state: &AppState,
    track_id: &str,
) -> Result<SoundcloudTrackPayload, String> {
    let snapshot = {
        let store = state
            .library
            .lock()
            .map_err(|_| "library store lock poisoned".to_string())?;
        store
            .load_soundcloud_lookup(track_id)
            .map_err(|error| error.to_string())?
    };

    match snapshot {
        Some(record) => Ok(build_lookup_payload(record)),
        None => Err(format!("track '{track_id}' not found in library")),
    }
}

fn build_lookup_payload(record: SoundcloudLookupRecord) -> SoundcloudTrackPayload {
    let SoundcloudLookupRecord {
        track_id,
        title,
        artist,
        soundcloud_id,
        permalink_url,
        raw_payload,
    } = record;

    let raw_value = raw_payload.unwrap_or(Value::Null);
    let tags = extract_lookup_tags(&raw_value);
    let artwork_url = extract_first_string(&raw_value, &["artwork_url", "artworkUrl"]);
    let duration_ms = raw_value
        .get("duration")
        .and_then(|value| value.as_i64())
        .or_else(|| {
            raw_value
                .get("full_duration")
                .and_then(|value| value.as_i64())
        });
    let liked_at = extract_first_string(&raw_value, &["liked_at", "likedAt"]);
    let playlist_id = extract_first_string(&raw_value, &["playlist_id", "playlistId"]);
    let playlist_position = raw_value
        .get("playlist_position")
        .or_else(|| raw_value.get("playlistPosition"))
        .and_then(|value| value.as_i64());
    let source = extract_first_string(&raw_value, &["source"]);

    let resolved_soundcloud_id = soundcloud_id.unwrap_or_else(|| track_id.clone());

    SoundcloudTrackPayload {
        track_id,
        soundcloud_id: resolved_soundcloud_id,
        title,
        artist,
        tags,
        permalink_url,
        artwork_url,
        duration_ms,
        liked_at,
        playlist_id,
        playlist_position,
        source,
        raw: raw_value,
    }
}

fn extract_lookup_tags(raw: &Value) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(tag_list) = raw.get("tag_list").and_then(|value| value.as_str()) {
        for tag in tag_list.split_whitespace() {
            let trimmed = tag.trim();
            if !trimmed.is_empty() {
                tags.push(trimmed.to_string());
            }
        }
    }

    if let Some(genre) = raw.get("genre").and_then(|value| value.as_str()) {
        let trimmed = genre.trim();
        if !trimmed.is_empty() && !tags.iter().any(|existing| existing == trimmed) {
            tags.push(trimmed.to_string());
        }
    }

    tags
}

fn extract_first_string(raw: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = raw.get(key).and_then(|value| value.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[tauri::command]
fn link_soundcloud_source(
    state: tauri::State<AppState>,
    record: SoundcloudSourceRecord,
) -> Result<(), String> {
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .link_soundcloud_source(&record)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_local_asset(
    state: tauri::State<AppState>,
    record: LocalAssetRecord,
) -> Result<(), String> {
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .record_local_asset(&record)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_missing_assets(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .list_missing_assets()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_library_status(
    state: tauri::State<AppState>,
    filter: Option<StatusFilter>,
) -> Result<LibraryStatusPage, String> {
    let filter = filter.unwrap_or_default();
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .list_library_status(&filter)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_discogs_candidates(
    state: tauri::State<AppState>,
    track_id: String,
) -> Result<Vec<DiscogsCandidateRecord>, String> {
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .list_discogs_candidates(&track_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_musicbrainz_candidates(
    state: tauri::State<AppState>,
    track_id: String,
) -> Result<Vec<MusicbrainzCandidateRecord>, String> {
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .list_musicbrainz_candidates(&track_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn import_rekordbox_library(
    state: tauri::State<'_, AppState>,
    db_path: String,
) -> Result<(), String> {
    let source_path = PathBuf::from(db_path);
    let import_path = source_path.clone();
    let tracks = async_runtime::spawn_blocking(move || load_tracks(&import_path))
        .await
        .map_err(|error| format!("failed to join rekordbox import task: {error}"))?
        .map_err(|error| error.to_string())?;

    {
        let mut library = state
            .library
            .lock()
            .map_err(|_| "library store lock poisoned".to_string())?;
        library
            .sync_rekordbox_tracks(&tracks)
            .map_err(|error| error.to_string())?;
    }

    let mut rekordbox_state = state
        .rekordbox
        .lock()
        .map_err(|_| "rekordbox state lock poisoned".to_string())?;

    if supports_auto_refresh(&source_path) {
        rekordbox_state.configure(source_path, state.library.clone());
    } else {
        rekordbox_state.disable();
    }

    Ok(())
}

fn register_media_shortcuts(app: &AppHandle) -> Result<(), tauri_plugin_global_shortcut::Error> {
    let shortcut_manager = app.global_shortcut();

    shortcut_manager.on_shortcuts(
        ["CmdOrCtrl+Alt+P", "MediaPlayPause"],
        |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                emit_media_event(app, MEDIA_TOGGLE_EVENT);
            }
        },
    )?;

    shortcut_manager.on_shortcuts(
        ["CmdOrCtrl+Alt+N", "MediaNextTrack"],
        |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                emit_media_event(app, MEDIA_NEXT_EVENT);
            }
        },
    )?;

    shortcut_manager.on_shortcuts(
        ["CmdOrCtrl+Alt+B", "MediaPreviousTrack"],
        |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                emit_media_event(app, MEDIA_PREVIOUS_EVENT);
            }
        },
    )?;

    shortcut_manager.on_shortcuts(["MediaPlay"], |app, _shortcut, event| {
        if matches!(event.state, ShortcutState::Pressed) {
            emit_media_event(app, MEDIA_PLAY_EVENT);
        }
    })?;

    shortcut_manager.on_shortcuts(["MediaPause"], |app, _shortcut, event| {
        if matches!(event.state, ShortcutState::Pressed) {
            emit_media_event(app, MEDIA_PAUSE_EVENT);
        }
    })?;

    Ok(())
}

pub(crate) fn emit_media_event(app: &AppHandle, event: &str) {
    let _ = app.emit_to(MAIN_WINDOW_LABEL, event, ());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            PluginBuilder::new("navigation-guard")
                .on_navigation(|_, url| {
                    let allowed = match url.scheme() {
                        "tauri" | "https" => true,
                        "http" => url.host_str() == Some("localhost"),
                        "about" => url.as_str() == "about:blank",
                        _ => false,
                    };
                    if !allowed {
                        eprintln!("blocked navigation to disallowed URL: {url}");
                    }
                    allowed
                })
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            open_external,
            refresh_soundcloud_likes,
            retry_discogs_lookup,
            retry_musicbrainz_lookup,
            list_discogs_candidates,
            list_musicbrainz_candidates,
            confirm_musicbrainz_match,
            upsert_track,
            link_soundcloud_source,
            record_local_asset,
            list_missing_assets,
            list_library_status,
            import_rekordbox_library
        ])
        .setup(|app| {
            register_media_shortcuts(&app.handle())
                .map_err(|error| -> Box<dyn Error> { Box::new(error) })?;
            let app_state = AppState::new(&app.handle())
                .map_err(|error| -> Box<dyn Error> { Box::new(error) })?;
            app.manage(app_state);
            app.manage(WindowState::default());
            let tray =
                setup_tray(&app.handle()).map_err(|error| -> Box<dyn Error> { Box::new(error) })?;
            app.manage(TrayState(tray));

            let handle = app.handle();

            let media_handle = handle.clone();
            handle.listen_any(MEDIA_STATE_EVENT, move |event| {
                if let Ok(payload) = serde_json::from_str::<MediaUpdatePayload>(event.payload()) {
                    if let Some(update) = MediaUpdate::from_payload(payload) {
                        handle_media_update(&media_handle, update);
                    }
                }
            });

            let theme_handle = handle.clone();
            handle.listen_any(THEME_CHANGE_EVENT, move |event| {
                if let Ok(payload) = serde_json::from_str::<ThemeChangePayload>(event.payload()) {
                    handle_theme_change(&theme_handle, payload);
                }
            });

            let like_handle = handle.clone();
            handle.listen_any(LIBRARY_LIKE_EVENT, move |event| {
                if let Ok(payload) = serde_json::from_str::<SoundcloudTrackPayload>(event.payload()) {
                    if let Some(state) = like_handle.try_state::<AppState>() {
                        let store = match state.library.lock() {
                            Ok(store) => store,
                            Err(_) => {
                                eprintln!(
                                    "[soundcloud-wrapper] failed to acquire library store lock for like update"
                                );
                                return;
                            }
                        };

                        let track_record = TrackRecord {
                            track_id: payload.track_id.clone(),
                            title: payload.title.clone(),
                            artist: payload.artist.clone(),
                            album: None,
                            discogs_release_id: None,
                            discogs_confidence: None,
                            musicbrainz_release_id: None,
                            musicbrainz_confidence: None,
                            musicbrainz_payload: None,
                        };
                        let source_record = SoundcloudSourceRecord {
                            track_id: payload.track_id.clone(),
                            soundcloud_id: payload.soundcloud_id.clone(),
                            permalink_url: payload.permalink_url.clone(),
                            raw_payload: payload.raw.clone(),
                        };

                        if let Err(error) =
                            store.sync_soundcloud_track(&track_record, &source_record)
                        {
                            eprintln!(
                                "[soundcloud-wrapper] failed to persist SoundCloud like update: {error}"
                            );
                        } else {
                            state.musicbrainz.queue_lookup(payload.clone());
                            state.discogs.queue_lookup(payload);
                        }
                    }
                }
            });

            let playlist_handle = handle.clone();
            handle.listen_any(LIBRARY_PLAYLIST_EVENT, move |event| {
                if let Ok(payload) = serde_json::from_str::<SoundcloudPlaylistPayload>(event.payload()) {
                    if let Some(state) = playlist_handle.try_state::<AppState>() {
                        let store = match state.library.lock() {
                            Ok(store) => store,
                            Err(_) => {
                                eprintln!(
                                    "[soundcloud-wrapper] failed to acquire library store lock for playlist update"
                                );
                                return;
                            }
                        };

                        for track in payload.tracks.into_iter() {
                            let track_record = TrackRecord {
                                track_id: track.track_id.clone(),
                                title: track.title.clone(),
                                artist: track.artist.clone(),
                                album: None,
                                discogs_release_id: None,
                                discogs_confidence: None,
                                musicbrainz_release_id: None,
                                musicbrainz_confidence: None,
                                musicbrainz_payload: None,
                            };
                            let source_record = SoundcloudSourceRecord {
                                track_id: track.track_id.clone(),
                                soundcloud_id: track.soundcloud_id.clone(),
                                permalink_url: track.permalink_url.clone(),
                                raw_payload: track.raw.clone(),
                            };

                            if let Err(error) =
                                store.sync_soundcloud_track(&track_record, &source_record)
                            {
                                eprintln!(
                                    "[soundcloud-wrapper] failed to persist SoundCloud playlist update: {error}"
                                );
                            } else {
                                state.musicbrainz.queue_lookup(track.clone());
                                state.discogs.queue_lookup(track);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .on_page_load(|window, _payload| {
            if let Err(error) = window.eval(include_str!("scripts/inject.js")) {
                eprintln!("failed to inject media bridge script: {error}");
            }
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                hide_main_window(&window.app_handle());
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let menu = MenuBuilder::new(app)
        .text(TRAY_MENU_TOGGLE, "Mostrar/Ocultar ventana")?
        .text(TRAY_MENU_HOME, "Ir a Inicio")?
        .separator()
        .text(TRAY_MENU_EXIT, "Salir")?
        .build()?;

    let mut tray_builder = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_TOGGLE => toggle_main_window(app),
            TRAY_MENU_HOME => go_home(app),
            TRAY_MENU_EXIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click { button, .. } if button == MouseButton::Left => {
                toggle_main_window(tray.app_handle());
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder = tray_builder.tooltip("SoundCloud Wrapper");
    tray_builder.build(app)
}

fn handle_media_update(app: &AppHandle, update: MediaUpdate) {
    if let Ok(mut manager) = app.state::<AppState>().media.lock() {
        manager.integration.update(&update);
        manager.cache.update(&update);
    }
}

fn handle_theme_change(app: &AppHandle, payload: ThemeChangePayload) {
    let theme_label = payload.theme.unwrap_or_else(|| "desconocido".into());
    let metadata = payload
        .metadata
        .map(|metadata| metadata.into_metadata())
        .or_else(|| {
            app.state::<AppState>()
                .media
                .lock()
                .ok()
                .and_then(|manager| manager.cache.metadata.clone())
        });

    let mut body = format!("Tema cambiado a {theme_label}.");
    if let Some(meta) = metadata {
        if let Some(title) = meta.title {
            let track_line = if let Some(artist) = meta.artist {
                format!("\nReproduciendo: {title} — {artist}")
            } else {
                format!("\nReproduciendo: {title}")
            };
            body.push_str(&track_line);
        }
    }

    if let Err(error) = app
        .notification()
        .builder()
        .title("Tema actualizado")
        .body(body)
        .show()
    {
        eprintln!("failed to show theme change notification: {error}");
    }
}

fn toggle_main_window(app: &AppHandle) {
    let hidden = app.state::<WindowState>().hidden.load(Ordering::SeqCst);
    if hidden {
        show_main_window(app);
    } else {
        hide_main_window(app);
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_window(MAIN_WINDOW_LABEL) {
        if let Err(error) = window.show() {
            eprintln!("failed to show window: {error}");
        }
        if let Err(error) = window.set_focus() {
            eprintln!("failed to focus window: {error}");
        }
    }
    app.state::<WindowState>()
        .hidden
        .store(false, Ordering::SeqCst);
}

fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_window(MAIN_WINDOW_LABEL) {
        if let Err(error) = window.hide() {
            eprintln!("failed to hide window: {error}");
        }
    }
    app.state::<WindowState>()
        .hidden
        .store(true, Ordering::SeqCst);
}

fn go_home(app: &AppHandle) {
    show_main_window(app);
    if let Some(window) = app.get_window(MAIN_WINDOW_LABEL) {
        if let Err(error) = window.emit(TRAY_HOME_EVENT, Value::Null) {
            eprintln!("failed to emit home event: {error}");
            if let Err(eval_error) =
                window.eval("window.location.href = 'https://soundcloud.com/';")
            {
                eprintln!("failed to navigate home: {eval_error}");
            }
        }
    }
}

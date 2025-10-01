mod library;
mod media;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use library::{LibraryStore, LocalAssetRecord, SoundcloudSourceRecord, TrackRecord};
use media::{MediaCache, MediaIntegration, MediaUpdate, MediaUpdatePayload, ThemeChangePayload};
use serde_json::{self, Value};
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

struct AppState {
    media: Mutex<MediaManager>,
    library: Arc<Mutex<LibraryStore>>,
}

struct MediaManager {
    integration: MediaIntegration,
    cache: MediaCache,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self, library::LibraryError> {
        let library = LibraryStore::initialize(app)?;

        Ok(Self {
            media: Mutex::new(MediaManager {
                integration: MediaIntegration::initialize(app),
                cache: MediaCache::default(),
            }),
            library: Arc::new(Mutex::new(library)),
        })
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
fn upsert_track(state: tauri::State<AppState>, record: TrackRecord) -> Result<(), String> {
    let store = state
        .library
        .lock()
        .map_err(|_| "library store lock poisoned".to_string())?;
    store
        .upsert_track(&record)
        .map_err(|error| error.to_string())
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
            upsert_track,
            link_soundcloud_source,
            record_local_asset,
            list_missing_assets
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

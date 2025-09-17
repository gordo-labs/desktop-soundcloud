use std::error::Error;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_shell::ShellExt;

const MAIN_WINDOW_LABEL: &str = "main";
const MEDIA_TOGGLE_EVENT: &str = "media://toggle";
const MEDIA_PLAY_EVENT: &str = "media://play";
const MEDIA_PAUSE_EVENT: &str = "media://pause";
const MEDIA_NEXT_EVENT: &str = "media://next";
const MEDIA_PREVIOUS_EVENT: &str = "media://previous";

#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|error| format!("invalid URL: {error}"))?;
    match parsed.scheme() {
        "http" | "https" => {
            let target = parsed.into_string();
            app.shell()
                .open(target, None)
                .map_err(|error| format!("failed to open URL externally: {error}"))
        }
        scheme => Err(format!("unsupported scheme '{scheme}'")),
    }
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

fn emit_media_event(app: &AppHandle, event: &str) {
    let _ = app.emit_to(MAIN_WINDOW_LABEL, event, ());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![open_external])
        .setup(|app| {
            register_media_shortcuts(&app.handle())
                .map_err(|error| -> Box<dyn Error> { Box::new(error) })?;
            Ok(())
        })
        .on_page_load(|window, _payload| {
            if let Err(error) = window.eval(include_str!("scripts/inject.js")) {
                eprintln!("failed to inject media bridge script: {error}");
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

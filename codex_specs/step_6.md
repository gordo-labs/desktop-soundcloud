Agrega integración por OS:
- Linux: usar MPRIS. Si hay crate o plugin Tauri disponible, intégralo; si no, expón un pequeño servicio Rust con mpris-player y sincroniza estado play/pause/track usando IPC.
- Windows: expón SMTC usando Windows crate (windows-rs) para SystemMediaTransportControls; mapea eventos a los handlers existentes.
- macOS: usa Now Playing / MPNowPlayingInfoCenter si es factible; sincroniza metadata desde MediaSession del WebView.
Entrega: código por plataforma con #[cfg(target_os = ...)] y notas si alguna parte queda como guía.
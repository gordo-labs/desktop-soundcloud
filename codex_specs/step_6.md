Add OS-specific integration:
- Linux: use MPRIS. If a Tauri crate or plugin exists, integrate it; otherwise, expose a small Rust service with `mpris-player` and sync play/pause/track state via IPC.
- Windows: expose SMTC using the Windows crate (`windows-rs`) for `SystemMediaTransportControls`; map events to the existing handlers.
- macOS: use Now Playing / `MPNowPlayingInfoCenter` if feasible; sync metadata from the WebView `MediaSession`.
Deliver: per-platform code with `#[cfg(target_os = ...)]` and notes if any part remains as guidance.

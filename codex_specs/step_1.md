Within the existing Tauri project:
1) Edit `tauri.conf.json` to:
   - set `app.sandbox = true` if supported by the current version; otherwise keep the secure defaults.
   - configure `tauri.security.csp` to allow `self` and `*.soundcloud.com`, `*.scdn.co`, `*.sndcdn.com`, `*.googleapis.com` strictly for scripts/media; block `eval`.
   - reduce the allowlist to the minimum: `app`, `shell` (only `openExternal` for http/https), `globalShortcut`, `notification`.
   - define windows: a single main window, resizable, title `"SoundCloud Wrapper"`.
2) Configure the list of permitted navigation URLs: `https://soundcloud.com` and the necessary subdomains.
3) Implement a modern Chrome-like User-Agent string when creating the WebView.
Return the final `tauri.conf.json` JSON and the Rust snippets required to set the User-Agent.

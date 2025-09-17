# Goal
Create a **lightweight desktop app that is 100% independent from Chrome** and loads SoundCloud inside the **system WebView** using **Tauri**. Include global shortcuts, system media controls, tray icon, external link handling, session persistence, and packaging for macOS/Windows/Linux. Keep playback **inside the WebView** to respect the Terms of Service.

---

# Scope
- **In scope:** WebView wrapper, media keys (play/pause/next/prev), tray, notifications, basic deep links, MPRIS (Linux), SMTC (Windows), persistence, installers.
- **Out of scope:** Downloads/offline mode, scraping streams outside the WebView, TOS bypass, Widevine.

---

# Architecture
- **Tauri (Rust):** backend, global shortcuts, IPC with the frontend.
- **Minimal frontend:** a view that embeds `https://soundcloud.com` and injected JS for `MediaSession` + fallback selectors.
- **OS integration:** media keys, tray, notifications.
- **Security:** CSP, `tauri.conf.json` with a minimal allowlist.

---

# Conventions
- Binary/app name: `SoundCloud Wrapper` (adjustable).
- Package ID: `com.example.soundcloudwrapper` (replace with your domain).
- Project folder: `soundcloud-wrapper-tauri/`.

---

# Step prompts (for copying/pasting into Codex)
Each prompt is **self-contained** and recalls the previous context.

## Step 0 — Create the base project
**Goal:** Clean Tauri project with the Vue/React/Svelte or vanilla template (choose **vanilla** for a minimal setup).

**Deliverables:** Project tree, initial `tauri.conf.json`, dev script.

**Prompt:**
```
Act as a setup assistant. Create a minimal Tauri project (Rust + vanilla frontend) named "soundcloud-wrapper-tauri".
- Use Node + Vite for the frontend (vanilla TS).
- Initialise Tauri with the latest stable version.
- Add npm scripts: dev, build, tauri:dev, tauri:build.
- Configure a proper .gitignore for Node/Tauri.
Output: commands to run and final file structure.
```


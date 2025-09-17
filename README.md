# SoundCloud Wrapper Desktop

## Project goal
SoundCloud Wrapper Desktop is a cross-platform application built with Tauri. It delivers a lightweight native experience for listening to SoundCloud inside the system WebView while integrating with media controls, the system tray, and native notifications without depending on external browsers.

## Architecture
- **Rust backend (Tauri 2):** Manages the application lifecycle, registers global shortcuts, controls the tray icon, injects a custom script when SoundCloud loads, and exposes a hardened `open_external` command that launches links in the default browser.
- **Lightweight frontend with Vite + TypeScript:** Packages a minimal interface that embeds `https://soundcloud.com` and shows a placeholder while the integration finishes loading.
- **Injection bridge:** The `inject.js` script tracks the `MediaSession` state, emits events back to Rust, intercepts external links so they open outside the app, and triggers SoundCloud UI buttons so playback commands stay in sync.
- **Platform-specific integrations:** The media module delegates to MPRIS (Linux), SMTC (Windows), and the macOS media APIs to reflect playback status and receive commands from the operating system.

## Requirements
### Common
- Node.js 18 or newer and npm (or pnpm/yarn) to run the Vite/Tauri scripts.
- Stable Rust and `tauri-cli` (installed automatically via npm).
- (Optional) A SoundCloud account to test sign-in within the WebView.

### Windows
- Windows 10/11 with the **WebView2 Runtime** installed (even if Microsoft Edge is already available).
- Microsoft Visual C++ Build Tools 2019 or newer.

### macOS
- macOS 10.15 Catalina or newer (per `tauri.conf.json`).
- Xcode Command Line Tools installed (`clang`, `swift`, and codesign utilities).

### Linux
- A distribution that ships WebKitGTK 4.1 (for example Ubuntu 22.04+ or Fedora 38+).
- Required packages: `libwebkit2gtk-4.1`, `libgtk-3-dev`, `libsoup-3.0`, `webkit2gtk-driver`, `libayatana-appindicator3` (declared as a dependency of the `.deb` package).

## Installation and usage
1. Clone the repository and enter the project root.
   ```bash
   git clone <url> desktop-soundcloud
   cd desktop-soundcloud/soundcloud-wrapper-tauri
   ```
2. Install JavaScript and Rust dependencies.
   ```bash
   npm install
   ```
3. Launch the development build with hot reload inside the native Tauri window.
   ```bash
   npm run tauri:dev
   ```
4. Create an installer for your platform (AppImage/Deb/RPM, MSI, or DMG) when you are ready to distribute.
   ```bash
   npm run tauri:build
   ```
5. Additional handy scripts:
   - `npm run dev`: Starts the Vite development server (useful when debugging the frontend alone).
   - `npm run build`: Bundles the frontend into `dist/`.
   - `npm run test`: Runs the Vitest suite.
   - `npm run generate:icons`: Regenerates icons from `src-tauri/icon.svg`.

See [`docs/how-to-build.md`](soundcloud-wrapper-tauri/docs/how-to-build.md) for platform-specific build tips and CI considerations.

## Available shortcuts and controls
The following shortcuts work even when the window is in the background (subject to OS permissions):
- `CmdOrCtrl + Alt + P` or the **Play/Pause** media key: toggle playback.
- `CmdOrCtrl + Alt + N` or the **Next Track** media key: next track.
- `CmdOrCtrl + Alt + B` or the **Previous Track** media key: previous track.
- **Play** media key: force playback.
- **Pause** media key: pause playback.

Shortcuts emit IPC events that trigger the SoundCloud controls inside the WebView.

## Privacy and security
- **Strict Content Security Policy:** The Tauri configuration applies a CSP without `unsafe-*`, disables drag-and-drop, and limits the main window.
- **Minimal allowlist:** Only core events and the `open_external` command are enabled, with additional schema and credential validation.
- **Navigation guard:** Any attempt to load disallowed schemes (such as `file://`) is blocked, and external links are forced to open in the system browser via `shell.open` with a constrained regular expression.
- **System integration:** Playback state and theme changes are relayed through native APIs without persisting sensitive data beyond the in-memory cache required to update media integrations.

## Limitations and legal notes
- SoundCloud Wrapper Desktop is not an official SoundCloud application; it simply re-exposes the web experience inside a native container.
- It does not implement track downloads, offline playback, or stream redistribution. Any attempt to extract audio outside the WebView goes against the project goal and may violate the SoundCloud Terms of Service.
- Use of the application must respect the SoundCloud Terms of Service and licensing. Share installers only in territories where SoundCloud is available and avoid modifying binaries to bypass content restrictions.
- Authentication occurs directly with SoundCloud’s official servers inside the WebView; no credentials or external metrics are collected.
- Before distributing signed builds, ensure you meet the legal requirements of each platform (signing certificates, macOS notarization, SoundCloud trademarks, etc.).

# SoundCloud Wrapper Desktop

## Project goal
SoundCloud Wrapper Desktop is a cross-platform application built with Tauri. It delivers a lightweight native experience for listening to SoundCloud inside the system WebView while integrating with media controls, the system tray, and native notifications without depending on external browsers.

## Architecture
- **SoundCloud bridge:** The preload/injection script (`inject.js`) mounts inside the embedded SoundCloud session, synchronises playback state through Tauri IPC, mirrors likes/playlist actions into the local library cache, and guards outbound links so they open in the system browser.
- **Discogs worker:** A Rust background task queue (`DiscogsService`) throttles requests to the Discogs API, reconciles candidate releases against the local store, and notifies the UI when manual intervention is required.
- **MusicBrainz service (alpha):** A companion Rust client (`MusicbrainzService`) is being introduced to mirror the Discogs enrichment flow. It requires first-party credentials and is currently optional in production builds while the API contract settles.
- **Rust/Tauri host:** Coordinates the application lifecycle, tray/menu integration, global media shortcuts, and hardened shell commands while exposing a typed IPC surface to the frontend.
- **Vite + TypeScript frontend:** Provides the minimal chrome around the SoundCloud WebView, renders the library tooling, and surfaces enrichment status coming from the Discogs/MusicBrainz background workers.
- **Platform-specific integrations:** The media module delegates to MPRIS (Linux), SMTC (Windows), and the macOS media APIs to reflect playback status and receive commands from the operating system.

## Requirements
### Common
- Node.js 18 or newer and npm (or pnpm/yarn) to run the Vite/Tauri scripts.
- Stable Rust and `tauri-cli` (installed automatically via npm).
- Rust `cargo` and system toolchains capable of compiling GTK/WebKit dependencies (Tauri downloads the platform WebView bindings as needed).
- (Optional) A SoundCloud account to test sign-in within the WebView.

### Windows
- Windows 10/11 with the **WebView2 Runtime** installed (even if Microsoft Edge is already available).
- Microsoft Visual C++ Build Tools 2019 or newer.
- Optional: Windows SDK signing tools (`signtool.exe`) when running the release scripts.

### macOS
- macOS 10.15 Catalina or newer (per `tauri.conf.json`).
- Xcode Command Line Tools installed (`clang`, `swift`, and codesign utilities).
- Optional: Full Xcode if you plan to notarise or staple DMGs in CI.

### Linux
- A distribution that ships WebKitGTK 4.1 (for example Ubuntu 22.04+ or Fedora 38+).
- Required packages: `libwebkit2gtk-4.1`, `libgtk-3-dev`, `libsoup-3.0`, `webkit2gtk-driver`, `libayatana-appindicator3` (declared as a dependency of the `.deb` package).
- Optional: GPG tooling (`gnupg`, `gpg`) when signing Linux bundles through the helper scripts.

## Configuration and secrets
The enrichment workers depend on authenticated calls in local builds and CI. Export the following variables (for example via `.env`, your shell profile, or CI secrets) before starting the desktop app or the test/build scripts:

| Variable | Purpose |
| --- | --- |
| `MUSICBRAINZ_APP_NAME` | Application name sent in the MusicBrainz user agent header. |
| `MUSICBRAINZ_APP_VERSION` | Semantic version advertised to MusicBrainz. |
| `MUSICBRAINZ_APP_CONTACT` | Contact e-mail or URL associated with the MusicBrainz application. |
| `MUSICBRAINZ_TOKEN` | Personal access token used for authenticated MusicBrainz lookups. |

See [`docs/musicbrainz-credentials.md`](soundcloud-wrapper-tauri/docs/musicbrainz-credentials.md) for step-by-step guidance on creating and storing these credentials for both local development and automation runners.

Release automation scripts consume the usual platform-specific secrets documented in [`docs/release-signing.md`](soundcloud-wrapper-tauri/docs/release-signing.md): Apple notarisation credentials (`APPLE_IDENTITY`, `APPLE_TEAM_ID`, `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`), Windows code-signing certificates (`SIGNING_CERTIFICATE_PATH`, `SIGNING_CERTIFICATE_PASSWORD`, `TIMESTAMP_URL`), and optional Linux signing keys (`LINUX_SIGNING_KEY_ID`).

## CI-equivalent test and build pipeline
To mirror the continuous-integration workflow locally, run the commands below from `soundcloud-wrapper-tauri/` after installing dependencies:

```bash
npm run test
cargo test --workspace --manifest-path src-tauri/Cargo.toml
npm run tauri:build
```

This sequence executes the Vitest suite, runs the Rust workspace tests (matching the CI manifest path), and produces signed release bundles for the current platform.

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

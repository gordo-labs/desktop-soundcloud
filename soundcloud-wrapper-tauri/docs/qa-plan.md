# QA plan: SoundCloud Wrapper

## General preparation
- [ ] Verify that the build under test matches the final production configuration (permissions, allowed domains, signing if applicable).
- [ ] Ensure the test environment has a stable internet connection and access to SoundCloud test accounts (at least one free account).
- [ ] Clear previous app data (Tauri data directory) before starting cold-start and login tests.
- [ ] Prepare measurement tools:
  - [ ] Stopwatch or system utility to measure launch times (<2s) from launch until the UI is interactive.
  - [ ] System monitor (Activity Monitor, Task Manager, System Monitor) to log idle memory usage right after launch.
  - [ ] Console or app log collector to capture errors.

## 1. Cold start and idle consumption
- [ ] Remove previous user data.
- [ ] Launch the app from a "cold" state (no background processes) and measure the time until the UI is ready. Confirm it is < 2 seconds.
- [ ] Record idle memory usage (no interaction) immediately after SoundCloud loads; document the value and capture screenshots.
- [ ] Close and repeat three times to validate consistency.

## 2. Login and persistence
- [ ] Sign in with the test account from the WebView (SoundCloud).
- [ ] Confirm the session is maintained while the app is open (reload the view and verify it remains authenticated).
- [ ] Close the app completely, relaunch, and verify the session persists (no need to re-enter credentials).
- [ ] Validate that logging out manually clears the session after restart.

## 3. Playback controls
- [ ] Play a track and check that play/pause works via the WebView UI.
- [ ] Validate keyboard shortcuts defined by the app (play/pause/next/previous) while the window is focused.
- [ ] Test OS-level media keys while the app is in the background and confirm they control playback.
- [ ] Verify playback state changes are reflected inside SoundCloud (progress bar, title).

## 4. External links
- [ ] From an external link inside SoundCloud (for example, an artist’s "Twitter" link), confirm it opens in the system’s default browser and not in the WebView.
- [ ] Test internal links (tracks, playlists) and ensure they open inside the WebView.
- [ ] Review logs to confirm only the domains configured as external are allowed.

## 5. System tray
- [ ] Minimise the app to the tray and check that the main window disappears from the dock/taskbar.
- [ ] From the tray icon, restore the window and verify it keeps state and playback.
- [ ] Use the tray option to quit and confirm all app processes stop.

## 6. Track notifications
- [ ] Start playback and trigger a track change (manually or via a playlist).
- [ ] Confirm the system displays a notification with title, artist, and artwork.
- [ ] Validate that metadata updates in the notification/MPRIS/SMTC every time the track changes.
- [ ] Ensure notifications respect system permissions and do not appear duplicated.

## 7. Platform-specific integrations
### Linux
- [ ] Open an MPRIS-compatible player (for example `playerctl`, GNOME Media Control) and confirm the app appears with working controls.
- [ ] Test `playerctl play-pause`, `next`, `previous` commands and verify they reflect the correct state.

### Windows
- [ ] Play a track and open the SMTC panel (Win + P or the volume flyout) to confirm the app appears with metadata and controls.
- [ ] Validate that media keys show the system OSD and control playback.

### macOS
- [ ] If the Now Playing integration is implemented, open Control Center or the Touch Bar (if available) to check title/controls.
- [ ] Document if the integration is not available in the current build.

## 8. Security and restrictions
- [ ] Attempt to navigate to a non-allowed domain (capture log/alert). Confirm the app blocks the navigation.
- [ ] Validate that only APIs authorised by the Tauri allowlist are accessible from the WebView (attempt disallowed IPC calls and expect them to fail).
- [ ] Review the Content Security Policy and `tauri.conf.json` to ensure the allowed schemes, domains, and protocols match the requirements.

## 9. Wrap-up and reporting
- [ ] Collect metrics: launch times, memory consumption, media controller logs, screenshots.
- [ ] Document any bug or deviation, including steps to reproduce, environment, severity, and evidence.
- [ ] Verify once more that the app closes completely without leftover processes after finishing the tests.

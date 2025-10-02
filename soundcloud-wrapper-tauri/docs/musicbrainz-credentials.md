# MusicBrainz credential setup

SoundCloud Wrapper Desktop enriches the local library with release metadata fetched from the MusicBrainz API. This document explains how to obtain the required credentials and wire them into local development, CI jobs, and release builds.

## 1. Create a MusicBrainz application
1. Sign in to your [MusicBrainz account](https://musicbrainz.org/login).
2. Navigate to **Profile → Applications** and click **Add application**.
3. Fill the following fields:
   - **Application name**: a descriptive identifier such as `SoundCloud Wrapper Desktop`.
   - **Application version**: follow [SemVer](https://semver.org/) (for example `0.5.0`).
   - **Contact method**: a monitored e-mail address or URL the MusicBrainz team can use to reach you.
4. Submit the form and note the generated token. You can regenerate it later if it is compromised.

## 2. Required environment variables
The Rust `MusicbrainzService` reads four environment variables at startup:

| Variable | Description |
| --- | --- |
| `MUSICBRAINZ_APP_NAME` | Propagated to the user-agent string when calling MusicBrainz. |
| `MUSICBRAINZ_APP_VERSION` | Advertised application version (match the release tag where possible). |
| `MUSICBRAINZ_APP_CONTACT` | Contact value configured when creating the application. |
| `MUSICBRAINZ_TOKEN` | Personal access token returned by MusicBrainz. |

Set these variables before running `npm run tauri:dev`, the automated tests, or any of the release scripts. Missing values disable MusicBrainz lookups and surface warnings in the application logs.

### Local development
Create a `.env.local` (ignored by Git) in `soundcloud-wrapper-tauri/` or export the values via your shell profile:

```bash
export MUSICBRAINZ_APP_NAME="SoundCloud Wrapper Desktop"
export MUSICBRAINZ_APP_VERSION="0.5.0"
export MUSICBRAINZ_APP_CONTACT="dev@example.com"
export MUSICBRAINZ_TOKEN="paste-token-here"
```

Reload your terminal session before running the desktop app so the environment variables propagate to Tauri.

### Continuous integration
Store the same values as encrypted secrets in your CI provider. When mirroring the project pipeline (`npm run test && cargo test --workspace --manifest-path src-tauri/Cargo.toml && npm run tauri:build`), export the variables in the job definition so both Vitest and the Rust integration tests can reach MusicBrainz.

### Release scripts
The helper scripts in `scripts/` automatically forward the environment variables to `cargo tauri build`. Ensure the values are present alongside platform-specific signing credentials:

- `scripts/build-macos.sh` respects `APPLE_IDENTITY`, `APPLE_TEAM_ID`, and MusicBrainz variables during the notarised build.
- `scripts/build-windows.ps1` consumes Windows signing secrets (`SIGNING_CERTIFICATE_PATH`, `SIGNING_CERTIFICATE_PASSWORD`, `TIMESTAMP_URL`) in addition to the MusicBrainz variables for metadata lookups.
- `scripts/build-linux.sh` optionally signs the generated packages when `LINUX_SIGNING_KEY_ID` is set and keeps MusicBrainz lookups enabled by inheriting the exported variables.

For shared runners, prefer injecting the variables via secure secret managers rather than storing them in plaintext files.

# Distributing and signing binaries

This document summarises the recommended process for signing and notarising the packages produced by the scripts in `scripts/`.

## macOS (Developer ID)

1. **Prepare certificates**
   - Obtain a *Developer ID Application* certificate from your Apple Developer account and add it to the system keychain.
   - Export the certificate and private key as a `.p12` file if you plan to automate the process in CI.
2. **Important environment variables**
   - `APPLE_IDENTITY`: exact certificate name (for example `Developer ID Application: ACME Corp (TEAMID)`).
   - `APPLE_TEAM_ID`: Apple Developer Team ID.
   - `APPLE_ID` and `APPLE_APP_SPECIFIC_PASSWORD`: credentials required to submit the binary for automatic notarisation.
   - Optional: `APPLE_NOTARYTOOL_PROFILE` if you use a profile configured via `xcrun notarytool store-credentials`.
3. **Build and sign**
   - Run `./scripts/build-macos.sh` on a macOS host. The script forwards `APPLE_IDENTITY` and `APPLE_TEAM_ID` to the variables expected by Tauri (`TAURI_SIGNING_IDENTITY` and `TAURI_APPLE_TEAM_ID`).
   - Packaging produces a `.app` bundle and a `.dmg` under `src-tauri/target/release/bundle/dmg/`.
4. **Notarise**
   - After building, upload the `.dmg` with `xcrun notarytool submit <path>.dmg --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --wait`.
   - Staple the ticket: `xcrun stapler staple <path>.dmg`.

## Windows (MSI + SignTool)

1. **Prepare the certificate**
   - Acquire a code-signing certificate (EV preferred) and export a `.pfx` file with its password.
   - Install the *Windows SDK Signing Tools* (`signtool.exe`).
2. **Variables and parameters**
   - `SIGNING_CERTIFICATE_PATH`: path to the `.pfx` file.
   - `SIGNING_CERTIFICATE_PASSWORD`: password for the `.pfx` file.
   - `TIMESTAMP_URL` (optional): timestamping service URL. The script defaults to `http://timestamp.digicert.com`.
3. **Build and sign**
   - Execute `powershell.exe -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1`.
   - After `cargo tauri build --bundles msi`, the script locates the most recent MSI and signs it with `signtool sign /fd SHA256 /tr <timestamp> /td SHA256`.
4. **Verification**
   - Use `signtool verify /pa <path>.msi` to confirm the signature.

## Linux (AppImage/Deb/RPM)

1. **Optional GPG key**
   - Import your key: `gpg --import private.key`.
   - Choose the identifier (fingerprint or email) that will be used to sign.
2. **Optional variable**
   - `LINUX_SIGNING_KEY_ID`: fingerprint or UID of the key used for `gpg --detach-sign`.
3. **Build**
   - Run `./scripts/build-linux.sh`. Packages will be created in `src-tauri/target/release/bundle/{appimage,deb,rpm}/`.
   - If `LINUX_SIGNING_KEY_ID` is defined, each artefact generates a corresponding `.sig` file.
4. **Publishing**
   - For APT/YUM repositories, publish the signatures alongside the artefacts and ensure the public key is available to users.

## General best practices

- Run `npm ci` and `cargo tauri build` from a clean tree to avoid stale artefacts.
- Store certificates and passwords in a secure manager and use encrypted secrets in CI.
- Automate uploading of signed artefacts to your CDN or release repository to eliminate manual handling.

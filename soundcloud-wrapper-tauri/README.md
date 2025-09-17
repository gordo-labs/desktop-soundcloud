# SoundCloud Wrapper — Base Scaffold

This repository contains the minimal starting point for the "SoundCloud Wrapper" desktop application built with **Tauri 2** and a **Vite + TypeScript** frontend without additional frameworks.

## Commands used to bootstrap

```bash
npm create tauri-app@latest soundcloud-wrapper-tauri -- --template vanilla-ts
cd soundcloud-wrapper-tauri
npm install
```

## Available scripts

- `npm run dev`: Starts the Vite development server.
- `npm run build`: Generates the production build of the frontend.
- `npm run generate:icons`: Regenerates the multi-platform icons from `src-tauri/icon.svg`.
- `npm run tauri:dev`: Launches the Tauri application in development mode.
- `npm run tauri:build`: Packages the application for distribution.

## Project structure

```
soundcloud-wrapper-tauri/
├── index.html
├── package.json
├── src/
│   ├── assets/
│   ├── main.ts
│   └── styles.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   └── main.rs
│   └── tauri.conf.json
└── vite.config.ts
```

## Application icons

The repository includes a base vector icon at `src-tauri/icon.svg`. During packaging the scripts automatically run `npm run generate:icons` to produce the platform-specific artefacts inside `src-tauri/icons/` (ignored in Git to avoid binaries). If you need a different design, replace the SVG and regenerate the icons.

## Distribution and signing

- `./scripts/build-macos.sh`: Packages a `.dmg` on macOS and forwards `APPLE_IDENTITY`/`APPLE_TEAM_ID` if they are set.
- `./scripts/build-windows.ps1`: Builds and optionally signs the `.msi` using `signtool` when a certificate is available.
- `./scripts/build-linux.sh`: Generates AppImage/Deb/RPM bundles and signs them with GPG if `LINUX_SIGNING_KEY_ID` is configured.

Check [`docs/release-signing.md`](docs/release-signing.md) for detailed code-signing and notarization steps per platform.

## Recommended IDE

- [VS Code](https://code.visualstudio.com/) with the [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) and [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extensions.

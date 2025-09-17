# SoundCloud Wrapper — Base Scaffold

Este repositorio contiene el punto de partida mínimo para la aplicación de escritorio "SoundCloud Wrapper" construida con **Tauri 2** y un frontend **Vite + TypeScript** sin frameworks.

## Comandos ejecutados

```bash
npm create tauri-app@latest soundcloud-wrapper-tauri -- --template vanilla-ts
cd soundcloud-wrapper-tauri
npm install
```

## Scripts disponibles

- `npm run dev`: ejecuta el servidor de desarrollo de Vite.
- `npm run build`: genera la build de producción del frontend.
- `npm run generate:icons`: regenera los iconos multiplataforma a partir de `src-tauri/icon.svg`.
- `npm run tauri:dev`: levanta la aplicación de Tauri en modo desarrollo.
- `npm run tauri:build`: empaqueta la aplicación para distribución.

## Estructura del proyecto

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

## Iconos de la aplicación

El repositorio incluye un icono base vectorial en `src-tauri/icon.svg`. Durante el empaquetado los scripts ejecutan automáticamente `npm run generate:icons` para producir los artefactos específicos de cada plataforma dentro de `src-tauri/icons/` (ignorados en Git para evitar binarios). Si necesitas un diseño distinto, sustituye el SVG y vuelve a generar los iconos.

## Distribución y firma

- `./scripts/build-macos.sh`: empaqueta un `.dmg` en macOS propagando `APPLE_IDENTITY`/`APPLE_TEAM_ID` si están definidos.
- `./scripts/build-windows.ps1`: crea y firma (opcional) el `.msi` usando `signtool` cuando hay certificado.
- `./scripts/build-linux.sh`: genera AppImage/Deb/RPM y firma con GPG si `LINUX_SIGNING_KEY_ID` está configurado.

Consulta `docs/release-signing.md` para los pasos detallados de codesign y notarización en cada plataforma.

## IDE recomendado

- [VS Code](https://code.visualstudio.com/) con las extensiones [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) y [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

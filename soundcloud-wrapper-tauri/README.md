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

Los binarios de iconos generados por el scaffold de Tauri no se incluyen en el repositorio para evitar archivos binarios en los PRs.
Antes de empaquetar ejecuta `npm exec tauri icon` (o `pnpm tauri icon`/`yarn tauri icon`) con tu arte final y se regenerará la carpeta `src-tauri/icons/` de manera local.

## IDE recomendado

- [VS Code](https://code.visualstudio.com/) con las extensiones [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) y [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

# Objetivo
Crear una app de escritorio **ligera y 100% independiente de Chrome** que cargue SoundCloud dentro de un **WebView del sistema** usando **Tauri**. Incluir atajos globales, controles multimedia del sistema, bandeja, manejo de enlaces externos, persistencia de sesión y empaquetado para macOS/Windows/Linux. Mantener reproducción **dentro del WebView** para respetar ToS.

---

# Alcance
- **Sí**: Wrapper WebView, media keys (play/pause/next/prev), bandeja, notificaciones, deep links básicos, MPRIS (Linux), SMTC (Windows), persistencia, instaladores.
- **No**: Descargas/offline, scraping de streams fuera del WebView, bypass de ToS, Widevine.

---

# Arquitectura
- **Tauri (Rust)**: backend, atajos globales, IPC con frontend.
- **Frontend mínimo**: una vista que embebe `https://soundcloud.com` y JS inyectado para `MediaSession` + selectores de fallback.
- **Integración SO**: media keys, tray, notificaciones. 
- **Seguridad**: CSP, `tauri.conf.json` con allowlist mínimo.

---

# Convenciones
- Nombre del binario/app: `SoundCloud Wrapper` (ajustable).
- Paquete id: `com.example.soundcloudwrapper` (sustituir dominio).
- Carpeta proyecto: `soundcloud-wrapper-tauri/`.

---

# Prompts por Step (para copiar/pegar a Codex)
Cada prompt es **autocontenido** y recuerda el contexto previo.

## Step 0 — Crear proyecto base
**Meta:** Proyecto Tauri limpio con plantilla Vue/React/Svelte o vanilla (elige **vanilla** para mínimo).

**Deliverables:** Árbol del proyecto, `tauri.conf.json` inicial, script de dev.

**Prompt:**
```
Actúa como un asistente de setup. Crea un proyecto mínimo de Tauri (Rust + frontend vanilla) llamado "soundcloud-wrapper-tauri".
- Usa Node + Vite para el frontend (vanilla TS).
- Inicializa Tauri con última versión estable.
- Añade scripts npm: dev, build, tauri:dev, tauri:build.
- Configura .gitignore adecuado para Node/Tauri.
Entrega: comandos a ejecutar y estructura final de archivos.
```
...
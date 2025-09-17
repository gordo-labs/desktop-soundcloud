Implementa controles multimedia:
1) En preload/inyección JS, si existe navigator.mediaSession, define handlers para play, pause, previoustrack, nexttrack que hagan click en los controles del reproductor.
2) Provee fallback robusto con selectores (ejemplos):
   - Play/Pause: document.querySelector('[aria-label="Play"]') || document.querySelector('[aria-label="Pause"]')
   - Next: document.querySelector('[aria-label="Next"]')
   - Prev: document.querySelector('[aria-label="Previous"]')
3) En Rust, registra global shortcuts (ej: CmdOrCtrl+Alt+P para toggle, +N next, +B prev) usando tauri-plugin-global-shortcut y envía comandos al frontend vía IPC.
4) Asegúrate que también respondan a teclas multimedia del teclado si el plugin/OS las emite.
Entrega: código Rust (setup, comandos/emit), JS inyectado y wiring en el frontend.
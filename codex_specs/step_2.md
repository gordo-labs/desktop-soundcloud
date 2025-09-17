In the vanilla TS Vite frontend, create a minimal view that operates entirely inside the Tauri WebView. On start, redirect to https://soundcloud.com/ and ensure the session persists between restarts (cookies + localStorage).
- Add logic so any `target="_blank"` opens via `shell.openExternal`.
- Provide the required `index.html` and `main.ts` code.

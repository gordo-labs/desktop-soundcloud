En el proyecto Tauri existente:
1) Edita tauri.conf.json para:
   - app.sandbox = true si disponible en versión; si no, conserva defaults seguros.
   - tauri.security.csp para permitir self y *.soundcloud.com, *.scdn.co, *.sndcdn.com, *.googleapis.com estrictamente para scripts/media; bloquea eval.
   - allowlist mínima: app, shell (openExternal solo para http/https), globalShortcut, notification.
   - windows: una sola ventana principal, resizable, title: "SoundCloud Wrapper".
2) Configura lista de URLs de navegación permitidas: https://soundcloud.com y subdominios necesarios.
3) Implementa set de User-Agent tipo Chrome moderno (ejemplo de string) en la creación del WebView.
Devuelve el JSON final de tauri.conf.json y snippets de Rust necesarios para establecer UA.
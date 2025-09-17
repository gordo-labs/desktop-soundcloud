En el frontend vanilla TS de Vite, crea una vista mínima que simplemente opere dentro de la webview de Tauri. Al iniciar, redirige a https://soundcloud.com/ y confirma que la sesión persiste entre reinicios (cookies + localStorage).
- Añade lógica para que cualquier target="_blank" se abra con shell.openExternal.
- Proporciona el código de index.html y main.ts necesarios.
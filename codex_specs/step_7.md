Endurece el proyecto:
- Refuerza CSP (sin 'unsafe-inline' ni 'unsafe-eval' si es posible; usa nonces/hashes solo si absolutamente necesario por SoundCloud).
- Bloquea protocolos no requeridos (file://).
- Revisa allowlist de APIs Tauri y limita shell.open a https/http con validación.
- Desactiva drag&drop de archivos si no se usa.
- Documenta un checklist de seguridad final.
Devuelve el JSON final y cambios de código.
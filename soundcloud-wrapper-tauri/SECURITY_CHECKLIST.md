# Security Checklist

- [x] CSP endurecida sin `unsafe-inline` ni `unsafe-eval`, con directivas explícitas para scripts, estilos, fuentes e iframes.
- [x] CSP de desarrollo independiente que permite WebSocket solo para `localhost` y mantiene las restricciones principales.
- [x] Arrastre y suelta de archivos deshabilitado en la ventana principal.
- [x] Navegación filtrada mediante un plugin que bloquea esquemas no permitidos, incluido `file://`.
- [x] Comando `open_external` validado para aceptar únicamente URLs `https` o `http` de desarrollo, sin credenciales embebidas.
- [x] Configuración del plugin `shell` restringida a esquemas `https?://` mediante regex.
- [x] Capabilidad IPC reducida a eventos y al comando personalizado `open_external`.
- [x] Documentación de este checklist incluida en el repositorio.

## Pasos de verificación sugeridos

1. Revisar `src-tauri/tauri.conf.json` para confirmar las directivas CSP, el `dragDropEnabled` y la configuración del plugin `shell`.
2. Verificar `src-tauri/src/lib.rs` para observar el plugin de guardia de navegación y la validación adicional del comando `open_external`.
3. Comprobar `src-tauri/capabilities/default.json` y `src-tauri/permissions/open-external.json` para validar el alcance mínimo de permisos expuestos al frontend.

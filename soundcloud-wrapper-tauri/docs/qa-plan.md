# Plan de QA: SoundCloud Wrapper

## Preparación general
- [ ] Verificar que la build bajo prueba coincide con la configuración final de producción (permisos, dominios permitidos, firma si aplica).
- [ ] Asegurar que el entorno de prueba tiene conexión estable a internet y acceso a cuentas de prueba de SoundCloud (al menos una cuenta gratuita).
- [ ] Limpiar datos previos de la aplicación (directorio de datos de Tauri) antes de iniciar pruebas de arranque en frío y login.
- [ ] Preparar herramientas de medición:
  - [ ] Cronómetro o utilidad del sistema para medir tiempos de lanzamiento (< 2 segundos) desde el arranque hasta que la UI sea interactiva.
  - [ ] Monitor del sistema (Activity Monitor, Task Manager, System Monitor) para registrar uso de memoria en idle justo después del lanzamiento.
  - [ ] Consola o recolector de logs de la app para capturar errores.

## 1. Arranque en frío y consumo en idle
- [ ] Eliminar datos de usuario previos.
- [ ] Lanzar la app desde un estado "frío" (sin procesos en segundo plano) y medir el tiempo hasta que la UI esté lista. Confirmar que es < 2 segundos.
- [ ] Registrar el uso de memoria en idle (sin interacción) inmediatamente después de cargar SoundCloud; documentar el valor y capturar capturas de pantalla.
- [ ] Cerrar y repetir tres veces para validar consistencia.

## 2. Login y persistencia
- [ ] Iniciar sesión con la cuenta de prueba desde el WebView (SoundCloud).
- [ ] Confirmar que la sesión se mantiene mientras la app está abierta (recargar la vista y verificar que sigue autenticada).
- [ ] Cerrar completamente la app, relanzar y verificar que la sesión persiste (sin necesidad de reingresar credenciales).
- [ ] Validar que cerrar sesión manualmente limpia la sesión tras reiniciar.

## 3. Controles de reproducción
- [ ] Reproducir un track y comprobar que play/pause funciona desde la UI del WebView.
- [ ] Validar atajos de teclado definidos por la app (play/pause/siguiente/anterior) con la ventana enfocada.
- [ ] Probar teclas multimedia a nivel de OS con la app en segundo plano y confirmar que controlan la reproducción.
- [ ] Verificar que los cambios de estado de reproducción se reflejan en SoundCloud (barra de progreso, título).

## 4. Enlaces externos
- [ ] Desde un enlace externo dentro de SoundCloud (por ejemplo, el "Twitter" de un artista), confirmar que se abre en el navegador predeterminado del sistema y no en el WebView.
- [ ] Probar enlaces internos (tracks, playlists) y asegurar que abren dentro del WebView.
- [ ] Revisar logs para confirmar que solo se permiten los dominios configurados como externos.

## 5. Bandeja del sistema (tray)
- [ ] Minimizar la app a la bandeja y comprobar que la ventana principal desaparece del dock/barra de tareas.
- [ ] Desde el ícono de la bandeja, restaurar la ventana y verificar que mantiene estado y reproducción.
- [ ] Usar la opción de la bandeja para salir y confirmar que todos los procesos de la app se detienen.

## 6. Notificaciones de track
- [ ] Iniciar reproducción y forzar un cambio de track (manual o vía playlist).
- [ ] Confirmar que el sistema muestra una notificación con título, artista y carátula.
- [ ] Validar que los metadatos se actualizan en la notificación/MPRIS/SMTC cada vez que cambia el track.
- [ ] Asegurar que las notificaciones respetan permisos del sistema y no aparecen duplicadas.

## 7. Integraciones específicas por plataforma
### Linux
- [ ] Open an MPRIS-compatible player (for example `playerctl`, GNOME Media Control) and confirm the app appears with working controls.
- [ ] Test `playerctl play-pause`, `next`, `previous` commands and verify they reflect the correct state.

### Windows
- [ ] Play a track and open the SMTC panel (Win + P or the volume flyout) to confirm the app appears with metadata and controls.
- [ ] Validate that media keys show the system OSD and control playback.

### macOS
- [ ] If the Now Playing integration is implemented, open Control Center or the Touch Bar (if available) to check title/controls.
- [ ] Document if the integration is not available in the current build.

## 8. Seguridad y restricciones
- [ ] Intentar navegar a un dominio no permitido ("dominio no permitido", capturar log/alerta). Confirmar que la app bloquea la navegación.
- [ ] Validar que solo las APIs autorizadas por la allowlist de Tauri son accesibles desde el WebView (intentar llamadas IPC no permitidas y esperar que fallen).
- [ ] Revisar la Content Security Policy y `tauri.conf.json` para asegurar que los esquemas, dominios y protocolos permitidos coinciden con los requisitos.

## 9. Cierre y reporte
- [ ] Recopilar métricas: tiempos de lanzamiento, consumo de memoria, logs del controlador multimedia, capturas de pantalla.
- [ ] Documentar cualquier bug o desviación, incluyendo pasos para reproducir, entorno, severidad y evidencia.
- [ ] Verificar nuevamente que la app cierra completamente sin procesos residuales al finalizar las pruebas.

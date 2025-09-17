# Plan de QA: SoundCloud Wrapper

## Preparación general
- [ ] Verificar que la build usada para pruebas incluya la configuración final de producción (permisos, dominios permitidos, firma si aplica).
- [ ] Asegurar que el entorno de prueba cuente con conexión a internet estable y acceso a cuentas de prueba de SoundCloud (mínimo una gratuita).
- [ ] Limpiar configuraciones previas de la app (carpeta de datos de Tauri) antes de comenzar las pruebas de arranque en frío y login.
- [ ] Preparar herramientas de medición:
  - [ ] Cronómetro o utilidad del sistema para medir tiempos de arranque (<2s) desde el lanzamiento hasta que la UI es interactiva.
  - [ ] Monitor del sistema (Activity Monitor, Task Manager, System Monitor) para registrar el consumo de memoria en idle justo tras el arranque.
  - [ ] Consola o registrador de logs de la app para capturar errores.

## 1. Arranque en frío y consumo en idle
- [ ] Eliminar datos previos de usuario.
- [ ] Lanzar la app desde un estado "frío" (sin procesos en memoria) y medir el tiempo hasta que la UI está lista. Confirmar que es < 2 segundos.
- [ ] Registrar la memoria usada en idle (sin interacción) inmediatamente después de cargar SoundCloud; documentar valor y capturas.
- [ ] Cerrar y repetir 3 veces para validar consistencia.

## 2. Login y persistencia
- [ ] Iniciar sesión con la cuenta de prueba desde el WebView (SoundCloud).
- [ ] Confirmar que la sesión se mantiene mientras la app está abierta (recargar la vista y validar que sigue autenticado).
- [ ] Cerrar completamente la app, relanzar y verificar que la sesión persiste (sin reingresar credenciales).
- [ ] Validar que un logout manual borra la sesión tras reinicio.

## 3. Controles de reproducción
- [ ] Reproducir un track y comprobar que play/pause funciona mediante el UI del WebView.
- [ ] Validar atajos de teclado definidos en la app (play/pause/next/prev) mientras la ventana tiene foco.
- [ ] Probar teclas multimedia globales del SO (hardware o software) con la app en segundo plano y confirmar que controlan el reproductor.
- [ ] Verificar que los estados de reproducción cambian correctamente dentro de SoundCloud (barra de progreso, título).

## 4. Enlaces externos
- [ ] Desde un enlace externo dentro de SoundCloud (por ejemplo, "Twitter" de un artista), confirmar que se abre el navegador predeterminado del sistema y no el WebView.
- [ ] Probar enlaces internos (pistas, playlists) y asegurar que se abren dentro del WebView.
- [ ] Revisar los logs para confirmar que solo se permiten los dominios configurados como externos.

## 5. Bandeja del sistema (tray)
- [ ] Minimizar la app a la bandeja y comprobar que la ventana principal desaparece del dock/menú de tareas.
- [ ] Desde el icono de la bandeja, restaurar la ventana y validar que mantiene estado y reproducción.
- [ ] Usar la opción de salir desde la bandeja y confirmar que finaliza todos los procesos de la app.

## 6. Notificaciones de track
- [ ] Activar la reproducción y desencadenar un cambio de pista (manual o lista de reproducción).
- [ ] Confirmar que se muestra una notificación del sistema con título, artista y carátula.
- [ ] Validar que la metadata se actualiza en la notificación/MPRIS/SMTC cada vez que cambia la pista.
- [ ] Verificar que las notificaciones respetan permisos del sistema y no aparecen duplicadas.

## 7. Integraciones específicas por plataforma
### Linux
- [ ] Abrir un reproductor compatible con MPRIS (por ejemplo, `playerctl`, GNOME Media Control) y confirmar que la app aparece con controles funcionales.
- [ ] Probar comandos `playerctl play-pause`, `next`, `previous` y verificar que reflejan el estado correcto.

### Windows
- [ ] Reproducir un track y abrir el panel SMTC (Win + P o icono de volumen) para comprobar que la app aparece con metadata y controles.
- [ ] Validar que las teclas multimedia muestran el OSD del sistema y controlan la reproducción.

### macOS
- [ ] Si la funcionalidad Now Playing está implementada, abrir el centro de control o barra táctil (si disponible) para verificar título/controles.
- [ ] Documentar si la integración no está disponible en la build actual.

## 8. Seguridad y restricciones
- [ ] Intentar navegar a un dominio no permitido (capturar log/alerta). Confirmar que la app bloquea la navegación.
- [ ] Validar que solo las APIs autorizadas por la allowlist de Tauri están accesibles desde el WebView (probar llamadas IPC no permitidas y esperar fallo).
- [ ] Revisar la configuración de Content Security Policy y `tauri.conf.json` para asegurar que los esquemas, dominios y protocolos permitidos coinciden con los requisitos.

## 9. Cierre y reporte
- [ ] Recopilar métricas: tiempos de arranque, consumo de memoria, logs de controladores multimedia, capturas de pantalla.
- [ ] Documentar cualquier bug o desviación, indicando pasos para reproducir, entorno, severidad y adjuntar evidencia.
- [ ] Validar nuevamente que la app se cierra completamente sin procesos residuales tras finalizar las pruebas.

# SoundCloud Wrapper Desktop

## Objetivo del proyecto
SoundCloud Wrapper Desktop es una aplicación multiplataforma construida con Tauri cuyo objetivo es ofrecer una experiencia nativa ligera para escuchar SoundCloud dentro del WebView del sistema, integrándose con los controles multimedia, bandeja del sistema y notificaciones de cada sistema operativo sin depender de navegadores externos.

## Arquitectura
- **Backend en Rust (Tauri 2):** gestiona el ciclo de vida de la aplicación, registra atajos globales y controla la bandeja del sistema, además de inyectar un script propio cuando la ventana carga SoundCloud y de exponer un comando seguro `open_external` para abrir enlaces en el navegador predeterminado.
- **Frontend ligero con Vite + TypeScript:** la interfaz empaquetada con Vite actúa como contenedor del sitio `https://soundcloud.com` y muestra un mensaje mínimo mientras se completa la integración.
- **Puente de inyección:** el script `inject.js` captura el estado de `MediaSession`, emite eventos hacia Rust, intercepta enlaces externos para abrirlos fuera de la app y controla los botones nativos de SoundCloud para mantener sincronizados los comandos de reproducción.
- **Integraciones específicas por plataforma:** el módulo de medios delega en MPRIS (Linux), SMTC (Windows) y la integración de macOS para reflejar el estado de reproducción y aceptar comandos del sistema.

## Requisitos
### Comunes
- Node.js 18 o superior y npm (o pnpm/yarn) para ejecutar los scripts de Vite/Tauri.
- Rust estable y el `tauri-cli` (se instala automáticamente vía npm).
- Acceso a una cuenta de SoundCloud (opcional) para probar login dentro del WebView.

### Windows
- Windows 10/11 con el **WebView2 Runtime** instalado (incluso si ya tienes Microsoft Edge).
- Microsoft Visual C++ Build Tools 2019 o superior.

### macOS
- macOS 10.15 Catalina o superior (según `tauri.conf.json`).
- Xcode Command Line Tools instaladas (incluye `clang`, `swift` y utilidades de codesign).

### Linux
- Distribución con soporte para WebKitGTK 4.1 (por ejemplo Ubuntu 22.04+, Fedora 38+).
- Paquetes requeridos: `libwebkit2gtk-4.1`, `libgtk-3-dev`, `libsoup-3.0`, `webkit2gtk-driver`, `libayatana-appindicator3` (esta última ya declarada como dependencia del paquete `.deb`).

## Instalación y ejecución
1. Clona este repositorio y entra en la carpeta raíz del proyecto.
   ```bash
   git clone <url> desktop-soundcloud
   cd desktop-soundcloud/soundcloud-wrapper-tauri
   ```
2. Instala dependencias de JavaScript y Rust.
   ```bash
   npm install
   ```
3. Ejecuta en modo desarrollo con recarga en caliente dentro de una ventana nativa de Tauri.
   ```bash
   npm run tauri:dev
   ```
4. Genera un instalador para tu plataforma (AppImage/Deb/RPM, MSI o DMG) cuando estés listo para distribuir.
   ```bash
   npm run tauri:build
   ```
5. Scripts adicionales útiles:
   - `npm run dev`: solo arranca el servidor de Vite (útil para depurar frontend).
   - `npm run build`: compila el frontend en `dist/`.
   - `npm run test`: ejecuta la suite de pruebas con Vitest.
   - `npm run generate:icons`: regenera los iconos a partir de `src-tauri/icon.svg`.

## Atajos y controles disponibles
Los siguientes atajos funcionan aunque la ventana esté en segundo plano (según permisos del sistema):
- `CmdOrCtrl + Alt + P` o tecla multimedia **Play/Pause**: alternar reproducción.
- `CmdOrCtrl + Alt + N` o tecla multimedia **Next Track**: pista siguiente.
- `CmdOrCtrl + Alt + B` o tecla multimedia **Previous Track**: pista anterior.
- Tecla multimedia **Play**: forzar reproducción.
- Tecla multimedia **Pause**: pausar reproducción.
Estos atajos emiten eventos IPC que activan los selectores de los controles de SoundCloud dentro del WebView.

## Privacidad y seguridad
- **Política de contenido estricta:** la configuración de Tauri aplica una CSP sin `unsafe-*`, deshabilita arrastrar y soltar y limita la ventana principal.
- **Allowlist mínima:** solo se habilitan los eventos del core y el comando `open_external`, definido con validaciones adicionales de esquema y credenciales.
- **Guardia de navegación:** se bloquea cualquier intento de cargar esquemas no permitidos (como `file://`) y los enlaces externos se fuerzan a abrirse en el navegador del sistema mediante `shell.open` con regex restringida.
- **Integración con el sistema:** los estados de reproducción y cambios de tema se notifican con APIs nativas sin persistir datos sensibles más allá del caché en memoria usado para actualizar integraciones multimedia.

## Limitaciones y notas legales
- SoundCloud Wrapper Desktop no es una aplicación oficial de SoundCloud; únicamente reexpone la versión web en un contenedor nativo.
- No implementa descarga de pistas, reproducción offline ni redistribución de streams; cualquier intento de extraer audio fuera del WebView va contra el objetivo del proyecto y puede vulnerar los Términos de Servicio de SoundCloud.
- El uso de la aplicación debe respetar los Términos de Servicio y las licencias de SoundCloud. Comparte instaladores solo en los territorios donde SoundCloud esté disponible y evita modificar los binarios para burlar restricciones de contenido.
- La autenticación se realiza directamente con los servidores oficiales de SoundCloud dentro del WebView; no se recolectan credenciales ni métricas externas.
- Antes de distribuir builds firmadas, asegúrate de cumplir con los requisitos legales de cada plataforma (certificados de firma, notarización en macOS, marcas registradas de SoundCloud, etc.).

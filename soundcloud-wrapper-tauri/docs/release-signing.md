# Distribución y firma de binarios

Este documento resume el proceso recomendado para firmar y notarizar los paquetes generados con los scripts de `scripts/`.

## macOS (Developer ID)

1. **Preparar certificados**
   - Obtén un certificado *Developer ID Application* desde tu cuenta de Apple Developer y añádelo al llavero del sistema.
   - Exporta el certificado y la clave privada como un `.p12` si usarás automatización en CI.
2. **Variables de entorno claves**
   - `APPLE_IDENTITY`: nombre exacto del certificado (por ejemplo `Developer ID Application: ACME Corp (TEAMID)`).
   - `APPLE_TEAM_ID`: Team ID de Apple Developer.
   - `APPLE_ID` y `APPLE_APP_SPECIFIC_PASSWORD`: credenciales necesarias si enviarás el binario a notarización automática.
   - Opcional: `APPLE_NOTARYTOOL_PROFILE` si usas un perfil configurado con `xcrun notarytool store-credentials`.
3. **Compilar y firmar**
   - Ejecuta `./scripts/build-macos.sh` en un host macOS. El script propagará `APPLE_IDENTITY` y `APPLE_TEAM_ID` a las variables esperadas por Tauri (`TAURI_SIGNING_IDENTITY` y `TAURI_APPLE_TEAM_ID`).
   - El empaquetado genera un `.app` y un `.dmg` en `src-tauri/target/release/bundle/dmg/`.
4. **Notarizar**
   - Tras la compilación, sube el `.dmg` con `xcrun notarytool submit <ruta>.dmg --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --wait`.
   - Añade el ticket: `xcrun stapler staple <ruta>.dmg`.

## Windows (MSI + SignTool)

1. **Preparar el certificado**
   - Consigue un certificado de firma de código (idealmente EV) y exporta un `.pfx` con su contraseña.
   - Instala las *Windows SDK Signing Tools* (`signtool.exe`).
2. **Variables y parámetros**
   - `SIGNING_CERTIFICATE_PATH`: ruta al `.pfx`.
   - `SIGNING_CERTIFICATE_PASSWORD`: contraseña del `.pfx`.
   - `TIMESTAMP_URL` (opcional): URL del servicio de sellado de tiempo. Por defecto el script usa `http://timestamp.digicert.com`.
3. **Compilar y firmar**
   - Ejecuta `powershell.exe -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1`.
   - Tras `cargo tauri build --bundles msi`, el script localizará el MSI más reciente y lo firmará con `signtool sign /fd SHA256 /tr <timestamp> /td SHA256`.
4. **Verificación**
   - Usa `signtool verify /pa <ruta>.msi` para comprobar la firma.

## Linux (AppImage/Deb/RPM)

1. **Clave GPG opcional**
   - Importa tu clave: `gpg --import private.key`.
   - Elige el identificador (fingerprint o correo) que se usará para firmar.
2. **Variable opcional**
   - `LINUX_SIGNING_KEY_ID`: fingerprint o UID de la clave que se usará para firmar con `gpg --detach-sign`.
3. **Compilar**
   - Ejecuta `./scripts/build-linux.sh`. Se crearán los paquetes en `src-tauri/target/release/bundle/{appimage,deb,rpm}/`.
   - Si `LINUX_SIGNING_KEY_ID` está definido, cada artefacto generará un archivo `.sig` asociado.
4. **Publicación**
   - Para repositorios APT/YUM, publica también las firmas y asegura que la clave pública esté disponible para los usuarios.

## Buenas prácticas generales

- Ejecuta `npm ci` y `cargo tauri build` en un árbol limpio para evitar artefactos antiguos.
- Conserva los certificados y contraseñas en un gestor seguro y usa secretos cifrados en CI.
- Automatiza la subida de artefactos firmados a tu CDN o repositorio de lanzamientos para evitar manipulaciones manuales.

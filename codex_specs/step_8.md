Configura empaquetado:
- Ajusta tauri.conf.json para iconos por plataforma y productName.
- Genera build scripts para macOS (dmg), Windows (msi) y Linux (AppImage/deb/rpm).
- Documenta proceso de codesign: macOS (Developer ID), Windows (signtool + cert), Linux (firma de paquetes opcional).
- Opcional: provee un workflow GitHub Actions para build multi-OS con caché Rust/Node.
Entrega: archivos de config y comandos.
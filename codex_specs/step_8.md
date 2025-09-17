Configure packaging:
- Adjust `tauri.conf.json` for per-platform icons and `productName`.
- Generate build scripts for macOS (dmg), Windows (msi), and Linux (AppImage/deb/rpm).
- Document the codesign process: macOS (Developer ID), Windows (signtool + certificate), Linux (optional package signing).
- Optional: provide a GitHub Actions workflow for multi-OS builds with Rust/Node caching.
Deliver: configuration files and commands.

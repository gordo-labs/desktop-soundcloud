# How to build SoundCloud Wrapper

This guide describes how to compile and package the desktop application from source on each supported platform. Follow the steps below after cloning the repository.

## 1. Common prerequisites

Install the shared tooling regardless of the operating system:

- **Node.js 18+** and npm (or pnpm/yarn) to run Vite and Tauri scripts.
- **Rust stable** toolchain. Install via [rustup](https://rustup.rs/) and keep it up to date (`rustup update`).
- **Tauri CLI**. It is installed automatically when running `npm install`, but you can also install it globally with `cargo install tauri-cli`.
- **SoundCloud account (optional)** for testing authentication inside the embedded WebView.

After installing the prerequisites, bootstrap dependencies:

```bash
cd soundcloud-wrapper-tauri
npm install
```

## 2. Platform-specific setup

### Windows
- Install the [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section) even if Microsoft Edge is already present.
- Install the **Microsoft Visual C++ Build Tools 2019+**. Selecting the "Desktop development with C++" workload provides the required compilers and Windows SDK.
- Optional: install the [Windows 11 SDK](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/) if you plan to build MSIX packages.

### macOS
- Use macOS 10.15 Catalina or newer (matching `tauri.conf.json`).
- Install **Xcode Command Line Tools** (`xcode-select --install`) for `clang`, `swift`, and codesign utilities.
- Optional: install full Xcode if you intend to notarise releases.

### Linux
- Use a distribution that provides WebKitGTK 4.1 (Ubuntu 22.04+, Fedora 38+, Arch, etc.).
- Install required packages. Example for Debian/Ubuntu:
  ```bash
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev webkit2gtk-driver libayatana-appindicator3-dev
  ```
- For Fedora:
  ```bash
  sudo dnf install webkitgtk4.1-devel gtk3-devel libsoup3-devel webkit2gtk3-jsc-devel libappindicator-gtk3
  ```
- Ensure your system locale is UTF-8 to avoid build script issues.

## 3. Development workflow

Start the live-reload environment while iterating on the UI or backend:

```bash
npm run tauri:dev
```

This command builds the Rust backend, launches the Tauri window, and proxies Vite for frontend assets.

To work only on the frontend, you can start the Vite dev server separately:

```bash
npm run dev
```

## 4. Production builds

When you are ready to create installable artefacts, run:

```bash
npm run tauri:build
```

Tauri will compile the Rust binary in release mode, bundle the frontend, and produce platform-specific installers under `src-tauri/target/release/bundle/`.

### Script helpers

The repository includes helper scripts that wrap the build and signing steps:

- `./scripts/build-macos.sh`: Builds a `.dmg` on macOS and forwards signing identities when `APPLE_IDENTITY` and `APPLE_TEAM_ID` are set.
- `./scripts/build-windows.ps1`: Builds an `.msi` on Windows and uses `signtool` automatically if signing variables are provided.
- `./scripts/build-linux.sh`: Builds AppImage/Deb/RPM packages and optionally signs them with GPG when `LINUX_SIGNING_KEY_ID` is defined.

Refer to [`docs/release-signing.md`](release-signing.md) for detailed signing and notarisation instructions.

## 5. Continuous integration tips

- Use `npm ci` instead of `npm install` in CI environments to ensure repeatable dependency trees.
- Cache the Rust `target` directory and the `~/.cargo` registry between runs to reduce build times.
- Run `cargo tauri build --bundles <type>` if you only need a subset of artefacts (for example `--bundles appimage` on Linux runners).
- Capture build logs and artefacts as CI outputs for easier debugging.

## 6. Troubleshooting

| Issue | Possible fix |
| --- | --- |
| `error: linker cc not found` | Ensure the C/C++ toolchain is installed (Visual C++ Build Tools on Windows, Xcode CLT on macOS, `build-essential` on Debian/Ubuntu). |
| `failed to download tauri-cli` | Check network connectivity or configure proxy settings via `npm config set proxy` and `https-proxy`. |
| WebView is blank on Linux | Verify that the correct WebKitGTK version is installed and that `LIBGL_ALWAYS_SOFTWARE=1` is not forced unintentionally. |
| WebView2 missing runtime error | Reinstall the Evergreen WebView2 Runtime and restart the application. |

If you encounter issues not covered here, consult the [Tauri documentation](https://tauri.app/) and include relevant logs when reporting bugs.

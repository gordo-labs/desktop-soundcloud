#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

pushd "$PROJECT_ROOT" >/dev/null

vitest_cache_dir="${PROJECT_ROOT}/.cache/vitest"
mkdir -p "$vitest_cache_dir"
export VITEST_CACHE_DIR="$vitest_cache_dir"

npm ci
npm run test
npm run build
cargo test --workspace --manifest-path src-tauri/Cargo.toml
cargo tauri build --bundles appimage deb rpm "$@"

if [[ -n "${LINUX_SIGNING_KEY_ID:-}" ]]; then
  BUNDLE_DIR="src-tauri/target/release/bundle"
  mapfile -t artifacts < <(find "$BUNDLE_DIR" -maxdepth 2 -type f \( -name '*.AppImage' -o -name '*.deb' -o -name '*.rpm' \ ))
  if (( ${#artifacts[@]} )); then
    for artifact in "${artifacts[@]}"; do
      gpg --batch --yes --detach-sign --local-user "$LINUX_SIGNING_KEY_ID" "$artifact"
    done
  fi
fi

popd >/dev/null

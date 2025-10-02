#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

export RUSTFLAGS="${RUSTFLAGS:-}"

pushd "$PROJECT_ROOT" >/dev/null

vitest_cache_dir="${PROJECT_ROOT}/.cache/vitest"
mkdir -p "$vitest_cache_dir"
export VITEST_CACHE_DIR="$vitest_cache_dir"

if [[ -n "${APPLE_IDENTITY:-}" ]]; then
  export TAURI_SIGNING_IDENTITY="$APPLE_IDENTITY"
fi

if [[ -n "${APPLE_TEAM_ID:-}" ]]; then
  export TAURI_APPLE_TEAM_ID="$APPLE_TEAM_ID"
fi

if [[ -n "${APPLE_ID:-}" ]]; then
  export TAURI_APPLE_ID="$APPLE_ID"
fi

if [[ -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]]; then
  export TAURI_APPLE_PASSWORD="$APPLE_APP_SPECIFIC_PASSWORD"
fi

npm ci
npm run test
npm run build
cargo test --workspace --manifest-path src-tauri/Cargo.toml
cargo tauri build --bundles dmg "$@"

popd >/dev/null

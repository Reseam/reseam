#!/usr/bin/env bash
set -euo pipefail
browser_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
project_root=$(cd -- "$browser_root/.." && pwd)
cd "$project_root"
: "${WASI_SDK_PATH:?Set WASI_SDK_PATH to the extracted WASI SDK 33 directory}"
if [[ ! -x "$WASI_SDK_PATH/bin/clang" ]]; then
  echo "WASI SDK clang was not found in $WASI_SDK_PATH/bin" >&2
  exit 1
fi
export CARGO_BUILD_JOBS=4
export CC_wasm32_wasip1="$WASI_SDK_PATH/bin/clang"
export AR_wasm32_wasip1="$WASI_SDK_PATH/bin/llvm-ar"
rustup target add wasm32-wasip1
cargo xtask regen patch-api
cargo xtask runtime
cargo build --locked -p reseam-sdk-browser -p reseam-browser-compression --target wasm32-wasip1 --profile browser
mkdir -p "$browser_root/build/runtime"
./gradlew :reseam-browser-host:hostJar :reseam-browser-host:browserRuntimeJar
cp target/wasm32-wasip1/browser/reseam_sdk_browser.wasm target/wasm32-wasip1/browser/reseam_browser_compression.wasm "$browser_root/build/runtime/"
cp patch-api/generated/browser/methods.json "$browser_root/build/runtime/"

bun "$browser_root/scripts/package-runtime.ts"

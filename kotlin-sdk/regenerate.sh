#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."

pushd "$PROJECT_ROOT/crates/patcher" >/dev/null
STITCH_SKIP_JNI_GLUE=1 boltffi generate kotlin
STITCH_SKIP_JNI_GLUE=1 boltffi generate header -o ../../kotlin-sdk/generated/jni
popd >/dev/null

"$SCRIPT_DIR/fix-generated.sh"

echo "Regenerated Kotlin SDK and JNI headers."

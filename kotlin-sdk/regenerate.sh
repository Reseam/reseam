#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."

pushd "$PROJECT_ROOT/crates/patcher" >/dev/null
RESEAM_SKIP_JNI_GLUE=1 boltffi generate kotlin
RESEAM_SKIP_JNI_GLUE=1 boltffi generate header -o ../../kotlin-sdk/generated/jni
popd >/dev/null

"$SCRIPT_DIR/fix-generated.sh"

echo "Regenerated Kotlin SDK and JNI headers."

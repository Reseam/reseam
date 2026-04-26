#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
JNI="$PROJECT_ROOT/sdk/generated/jni/jni_glue.c"
KOTLIN="$PROJECT_ROOT/sdk/generated/app/reseam/sdk/ReseamSdk.kt"

if [ ! -f "$JNI" ]; then
    echo "error: generated JNI glue not found: $JNI" >&2
    exit 1
fi

if [ ! -f "$KOTLIN" ]; then
    echo "error: generated Kotlin binding not found: $KOTLIN" >&2
    exit 1
fi

python3 - "$JNI" "$KOTLIN" <<'PY'
import sys
from pathlib import Path

jni_path = Path(sys.argv[1])
kotlin_path = Path(sys.argv[2])

jni = jni_path.read_text()
kotlin = kotlin_path.read_text()

if "fun boltffiFutureContinuationCallback(" in kotlin:
    print("Generated Kotlin has async continuation support; JNI glue does not need the sync-callback fix.")
    raise SystemExit(0)

if "_poll(" in jni or "SubscriptionHandle" in jni:
    raise SystemExit(
        "error: generated JNI glue contains async/stream polling but Kotlin has no continuation callback"
    )

old = '''    if (boltffi_lookup_global_class(env, "app/reseam/sdk/Native", &g_callback_class) != BOLTFFI_GLOBAL_CLASS_OK) {
        g_callback_class = NULL;
        return JNI_ERR;
    }
    if (!boltffi_lookup_static_method(env, g_callback_class, "boltffiFutureContinuationCallback", "(JB)V", &g_callback_method)) {
        (*env)->DeleteGlobalRef(env, g_callback_class);
        g_callback_class = NULL;
        g_callback_method = NULL;
        return JNI_ERR;
    }
'''

new = '''    /*
     * BoltFFI 0.24.1 emits this lookup whenever a callback trait exists, even
     * when the module has only synchronous callbacks. Kotlin correctly omits
     * boltffiFutureContinuationCallback unless async functions/streams/async
     * callbacks exist, so keep JNI_OnLoad limited to the sync callback setup.
     * Remove this once BoltFFI gates the JNI continuation lookup the same way
     * as the Kotlin Native template.
     */
'''

if new in jni:
    print(f"Fixed: {jni_path}")
    raise SystemExit(0)

if old not in jni:
    raise SystemExit(
        "error: BoltFFI JNI continuation lookup template changed; update sdk/fix-generated.sh"
    )

jni_path.write_text(jni.replace(old, new))
print(f"Fixed: {jni_path}")
PY

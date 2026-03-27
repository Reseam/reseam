#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GEN="$SCRIPT_DIR/generated/dev/stitch/patch/StitchPatcher.kt"
DST="$SCRIPT_DIR/src/main/kotlin/dev/stitch/patch/StitchPatcher.kt"

if [ ! -f "$GEN" ]; then
    echo "error: generated file not found: $GEN" >&2
    exit 1
fi

cp "$GEN" "$DST"

# BoltFFI bug: BoltFFIScope, async infrastructure, and continuation callback are emitted
# unconditionally even when no async functions exist. Remove them and their imports.
python3 -c "
import re

with open('$DST') as f:
    s = f.read()

# Remove BoltFFIScope object
s = re.sub(r'^object BoltFFIScope.*?^}\n', '', s, flags=re.MULTILINE | re.DOTALL)

# Remove async constants
s = re.sub(r'^private const val BOLTFFI_FUTURE_POLL_READY.*\n', '', s, flags=re.MULTILINE)
s = re.sub(r'^private const val BOLTFFI_FUTURE_POLL_WAKE.*\n', '', s, flags=re.MULTILINE)

# Remove BoltFFIHandleMap class
s = re.sub(r'^internal class BoltFFIHandleMap.*?^}\n', '', s, flags=re.MULTILINE | re.DOTALL)

# Remove boltffiContinuationMap
s = re.sub(r'^private val boltffiContinuationMap.*\n', '', s, flags=re.MULTILINE)

# Remove boltffiCallAsync function
s = re.sub(r'^internal suspend inline fun <T> boltffiCallAsync.*?^}\n', '', s, flags=re.MULTILINE | re.DOTALL)

# Remove boltffiFutureContinuationCallback inside Native object (indented, multi-brace)
s = re.sub(r'    @JvmStatic fun boltffiFutureContinuationCallback\b.*?\n    }\n', '', s, flags=re.DOTALL)

# Remove unused imports
for imp in [
    'kotlin.coroutines.Continuation',
    'kotlin.coroutines.resume',
    'kotlin.coroutines.resumeWithException',
    'kotlinx.coroutines.CancellableContinuation',
    'kotlinx.coroutines.suspendCancellableCoroutine',
    'java.util.concurrent.ConcurrentHashMap',
    'java.util.concurrent.atomic.AtomicBoolean',
    'java.util.concurrent.atomic.AtomicLong',
]:
    s = s.replace('import ' + imp + '\n', '')

with open('$DST', 'w') as f:
    f.write(s)
"

echo "Fixed: $DST"

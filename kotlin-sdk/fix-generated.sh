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

python3 -c "
with open('$DST') as f:
    s = f.read()

# Remove unused imports that BoltFFI emits unconditionally
for imp in [
    'java.util.concurrent.ConcurrentHashMap',
    'java.util.concurrent.atomic.AtomicBoolean',
    'java.util.concurrent.atomic.AtomicLong',
]:
    s = s.replace('import ' + imp + '\n', '')

# BoltFFI v0.22 bug: useWireBytes is referenced but not defined
if 'useWireBytes' in s and 'fun useWireBytes' not in s:
    # Insert after the FfiException class
    marker = 'class FfiException(val code: Int, message: String) : Exception(message)\n'
    helper = '''
private inline fun <T> useWireBytes(bytes: ByteArray, block: (java.nio.ByteBuffer) -> T): T {
    return block(java.nio.ByteBuffer.wrap(bytes).order(java.nio.ByteOrder.LITTLE_ENDIAN))
}
'''
    s = s.replace(marker, marker + helper)

with open('$DST', 'w') as f:
    f.write(s)
"

echo "Fixed: $DST"

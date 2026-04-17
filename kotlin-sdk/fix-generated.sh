#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GEN="$SCRIPT_DIR/generated/app/reseam/patch/ReseamPatcher.kt"
DST="$SCRIPT_DIR/src/main/kotlin/app/reseam/patch/ReseamPatcher.kt"

if [ ! -f "$GEN" ]; then
    echo "error: generated file not found: $GEN" >&2
    exit 1
fi

cp "$GEN" "$DST"

python3 -c "
with open('$DST') as f:
    s = f.read()

header = '// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>\n// SPDX-License-Identifier: GPL-3.0-or-later\n\n'
if not s.startswith('// SPDX-FileCopyrightText:'):
    s = header + s

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

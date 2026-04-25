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

python3 - "$DST" <<'PY'
import sys

dst = sys.argv[1]
with open(dst) as f:
    s = f.read()

header = '// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>\n// SPDX-License-Identifier: GPL-3.0-or-later\n\n'
if not s.startswith('// SPDX-FileCopyrightText:'):
    s = header + s

# Remove imports that this generated file does not actually use.
for imp, symbol in [
    ('java.util.concurrent.ConcurrentHashMap', 'ConcurrentHashMap'),
    ('java.util.concurrent.atomic.AtomicBoolean', 'AtomicBoolean'),
    ('java.util.concurrent.atomic.AtomicLong', 'AtomicLong'),
]:
    body = s.replace('import ' + imp + '\n', '')
    if symbol not in body:
        s = body

old_loader = '''        val vmName = System.getProperty("java.vm.name").orEmpty()
        val isAndroidRuntime =
            vmName.contains("dalvik", ignoreCase = true) ||
            vmName.contains("art", ignoreCase = true)
        if (isAndroidRuntime) {
            System.loadLibrary(fallbackLibrary)
        } else {
            loadDesktopLibraries(preferredLibrary, fallbackLibrary)
        }
'''
new_loader = '''        val vmName = System.getProperty("java.vm.name").orEmpty()
        val bootstrapMode = System.getProperty("reseam.native.bootstrap").orEmpty()
        val isAndroidRuntime =
            vmName.contains("dalvik", ignoreCase = true) ||
            vmName.contains("art", ignoreCase = true)
        if (isAndroidRuntime) {
            System.loadLibrary(fallbackLibrary)
        } else if (bootstrapMode != "host-registered") {
            loadDesktopLibraries(preferredLibrary, fallbackLibrary)
        }
'''
if old_loader in s:
    s = s.replace(old_loader, new_loader)
elif new_loader not in s:
    raise SystemExit('error: BoltFFI native loader template changed; update host-registered bootstrap integration')

with open(dst, 'w') as f:
    f.write(s)
PY

echo "Fixed: $DST"

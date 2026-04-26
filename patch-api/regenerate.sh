#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
REAL_CARGO="$(command -v cargo)"
CARGO_WRAPPER_DIR="$(mktemp -d)"
trap 'rm -rf "$CARGO_WRAPPER_DIR"' EXIT

cat >"$CARGO_WRAPPER_DIR/cargo" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if [[ "\$(basename "\$PWD")" == "boltffi_bindgen_type_resolution" && -f Cargo.toml ]] && ! grep -q '^\[workspace\]' Cargo.toml; then
    perl -0pi -e 's/\n\n\[dependencies\]/\n\n[workspace]\n\n[dependencies]/' Cargo.toml
fi

exec "$REAL_CARGO" "\$@"
EOF
chmod +x "$CARGO_WRAPPER_DIR/cargo"
export PATH="$CARGO_WRAPPER_DIR:$PATH"

pushd "$PROJECT_ROOT/crates/patcher" >/dev/null
RESEAM_SKIP_JNI_GLUE=1 boltffi generate kotlin
RESEAM_SKIP_JNI_GLUE=1 boltffi generate header -o ../../patch-api/generated/jni
popd >/dev/null

"$SCRIPT_DIR/fix-generated.sh"

echo "Regenerated Kotlin SDK and JNI headers."

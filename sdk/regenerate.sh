#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
ANDROID_API="${ANDROID_API:-24}"

find_android_clang() {
    local prefix="$1"
    local name="${prefix}${ANDROID_API}-clang"

    if command -v "$name" >/dev/null 2>&1; then
        command -v "$name"
        return
    fi

    if [ -n "${ANDROID_HOME:-}" ]; then
        local found
        found="$(find "$ANDROID_HOME/ndk" -path "*/toolchains/llvm/prebuilt/*/bin/$name" -type f 2>/dev/null | sort -V | tail -n 1)"
        if [ -n "$found" ]; then
            printf '%s\n' "$found"
            return
        fi
    fi

    echo "error: Android clang not found for $name; put the NDK llvm bin directory on PATH" >&2
    exit 1
}

link_android_jnilib() {
    local triple="$1"
    local abi="$2"
    local clang_prefix="$3"
    local clang
    clang="$(find_android_clang "$clang_prefix")"

    local build_dir="$SCRIPT_DIR/target/boltffi/android/$triple/release"
    local object_path="$build_dir/jni_glue.o"
    local export_script="$build_dir/exports.map"
    local source_path="$PROJECT_ROOT/sdk/generated/jni/jni_glue.c"
    local include_dir="$SCRIPT_DIR/dist/android/include"
    local library_path="$PROJECT_ROOT/target/$triple/release/libreseam_sdk.a"
    local abi_dir="$PROJECT_ROOT/sdk/jniLibs/$abi"
    local output_path="$abi_dir/libreseam-sdk.so"

    if [ ! -f "$library_path" ]; then
        echo "error: built Android static library not found: $library_path" >&2
        exit 1
    fi

    mkdir -p "$build_dir" "$abi_dir"
    "$clang" -c -fPIC -O3 -I "$include_dir" "$source_path" -o "$object_path"
    cat >"$export_script" <<'MAP'
{
    global:
        Java_*;
        JNI_OnLoad*;
        JNI_OnUnload*;
        boltffi_*;
    local:
        *;
};
MAP
    "$clang" \
        -shared \
        -o "$output_path" \
        "$object_path" \
        -Wl,--whole-archive "$library_path" -Wl,--no-whole-archive \
        -Xlinker --version-script -Xlinker "$export_script" \
        -Wl,--gc-sections \
        -lm -llog -ldl
}

pack_android_jnilibs() {
    link_android_jnilib "aarch64-linux-android" "arm64-v8a" "aarch64-linux-android"
    link_android_jnilib "armv7-linux-androideabi" "armeabi-v7a" "armv7a-linux-androideabi"
    link_android_jnilib "i686-linux-android" "x86" "i686-linux-android"
    link_android_jnilib "x86_64-linux-android" "x86_64" "x86_64-linux-android"
}

pushd "$SCRIPT_DIR" >/dev/null
boltffi build android --release
boltffi generate kotlin
"$SCRIPT_DIR/fix-generated.sh"
boltffi generate header
pack_android_jnilibs
popd >/dev/null

echo "Regenerated Reseam SDK bindings and Android jniLibs."

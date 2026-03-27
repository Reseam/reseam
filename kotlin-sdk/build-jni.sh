#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
JNI_SRC="$SCRIPT_DIR/generated/jni/jni_glue.c"
HEADER_DIR="$SCRIPT_DIR/generated/jni"
OUTPUT_DIR="$PROJECT_ROOT/target/debug"

if [ -z "${JAVA_HOME:-}" ]; then
    echo "error: JAVA_HOME not set" >&2
    exit 1
fi

if [ ! -f "$JNI_SRC" ]; then
    echo "error: JNI glue not found at $JNI_SRC" >&2
    echo "Run 'cd crates/patcher && boltffi generate header -o ../../kotlin-sdk/generated/jni' first" >&2
    exit 1
fi

CDYLIB="$OUTPUT_DIR/libstitch_patcher.so"
if [ ! -f "$CDYLIB" ]; then
    echo "error: cdylib not found at $CDYLIB" >&2
    echo "Run 'JAVA_HOME=$JAVA_HOME cargo build -p stitch-patcher' first" >&2
    exit 1
fi

OS="$(uname -s)"
case "$OS" in
    Linux)
        JNI_INCLUDE="$JAVA_HOME/include/linux"
        EXT="so"
        RPATH_FLAG="-Wl,-rpath,\$ORIGIN"
        ;;
    Darwin)
        JNI_INCLUDE="$JAVA_HOME/include/darwin"
        EXT="dylib"
        RPATH_FLAG="-Wl,-rpath,@loader_path"
        ;;
    *)
        echo "error: unsupported OS: $OS" >&2
        exit 1
        ;;
esac

echo "Compiling JNI glue..."
cc -c -fPIC \
    -I"$HEADER_DIR" \
    -I"$JAVA_HOME/include" \
    -I"$JNI_INCLUDE" \
    -w \
    -o "$OUTPUT_DIR/jni_glue.o" \
    "$JNI_SRC"

echo "Linking libstitch_patcher_jni.$EXT..."
cc -shared \
    -o "$OUTPUT_DIR/libstitch_patcher_jni.$EXT" \
    "$OUTPUT_DIR/jni_glue.o" \
    -L"$OUTPUT_DIR" -lstitch_patcher \
    "$RPATH_FLAG"

rm -f "$OUTPUT_DIR/jni_glue.o"
echo "Built: $OUTPUT_DIR/libstitch_patcher_jni.$EXT"

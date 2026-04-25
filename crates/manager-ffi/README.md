# Reseam Manager FFI

BoltFFI surface for manager applications that call `reseam-library`.

This crate exposes a small JSON-based API plus a progress callback:

- `inspect_apk_json`
- `inspect_json`
- `patch_json`
- `PatchEventSink`

## Android Build

Install Rust Android targets:

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  x86_64-linux-android \
  i686-linux-android
```

Set the Android NDK toolchain on `PATH`. Adjust the NDK version if needed:

```bash
export ANDROID_NDK_BIN="$ANDROID_HOME/ndk/29.0.14206865/toolchains/llvm/prebuilt/linux-x86_64/bin"
export PATH="$ANDROID_NDK_BIN:$PATH"
```

Check the Android target:

```bash
cargo check -p reseam-manager-ffi --target aarch64-linux-android
```

Generate Kotlin bindings and package Android `jniLibs`:

```bash
cd crates/manager-ffi
boltffi pack android --release
```

Outputs are written to:

```text
manager-sdk/generated/
manager-sdk/jniLibs/
```

## Patcher Host Requirement

Android hosts must install a classloader before inspecting or patching bundles:

```kotlin
AndroidPatchHost.setClassLoader(patchClassLoader)
```

Call `AndroidPatchHost.clearClassLoader()` when the host no longer needs that loader.

The classloader must be able to resolve the Reseam SDK and patch classes. Desktop hosts continue to use the existing JVM `URLClassLoader` path.

## Verification

From the workspace root:

```bash
cargo check -p reseam-patcher
cargo check -p reseam-manager-ffi
cargo check --workspace
cd kotlin-sdk && ./gradlew compileKotlin
```

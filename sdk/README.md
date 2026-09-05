# Reseam SDK

Application integration SDK for Reseam clients.

This crate exposes Rust service APIs plus a small BoltFFI JSON API and progress callback:

- `inspect_json`
- `patch_json`
- `PatchEventSink`

Requests and responses are the types in `dto.rs` serialized with serde. A request names the bundle signers it trusts under `trust.keys`; the engine trusts nobody on its own.

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

Generate Kotlin bindings and package Android `jniLibs`:

```bash
cargo xtask regen sdk
```

Outputs are written to `sdk/generated/` and `sdk/jniLibs/`. They are build products, not sources.

## Desktop Build

```bash
JAVA_HOME=/path/to/jdk cargo xtask jni-host
```

Builds `target/release/libreseam_sdk_jni.so` for the current host.

## Publishing

The Kotlin packages are built by the Gradle project at the workspace root:

```bash
./gradlew publishToMavenLocal -PreseamSdkVersion=0.3.0
```

- `app.reseam:reseam-sdk` for managers (Kotlin Multiplatform, Android and JVM)
- `app.reseam:reseam-patch-sdk` for patch authors

CI publishes both to the Reseam Maven registry on every `v*` tag, with the version taken from the tag.

## Patcher Host Requirement

Android hosts must install a classloader before inspecting or patching bundles:

```kotlin
ReseamAndroidHost.setClassLoader(classLoader)
```

The classloader must be able to resolve the Reseam SDK and patch classes. Desktop hosts need nothing: the engine attaches to the JVM it was loaded into.

## Verification

From the workspace root:

```bash
cargo check --workspace
./gradlew build
```

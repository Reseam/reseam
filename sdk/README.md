# Reseam SDK

Application integration SDK for Reseam clients.

This crate is the Rust service; the CLI calls it directly. `sdk/native` exports it through BoltFFI, with the types from `crates/model`:

- `inspect(InspectRequest): InspectResponse`
- `patch(PatchRequest, onEvent): PatchOutcome`, with progress delivered as `RunEvent`
- `ApkInspection(apkPath, splitPaths)`: `metadata()`, `basePath()`, `splitPaths()`, `applicationIcon()`, `close()`
- `encodeSelection` / `decodeSelection` and `encodePatchMetadata` / `decodePatchMetadata`

Failures throw `SdkError`, which carries a typed `Problem`. A request names the bundle signers it trusts under `trust.keys`; the engine trusts nobody on its own.

Calls are synchronous; run them off the main thread. `onEvent` runs on the calling thread and must not call back into the SDK. Component paths from an `ApkInspection` stay valid until it is closed.

Store selections and patch metadata with the encode functions. They write the serde JSON schema, which does not change with BoltFFI's wire format.

`sdk/native` is a separate crate because BoltFFI exports the `#[export]` functions of every direct dependency, and this crate depends on the patcher. See [BoltFFI integration](../docs/bindings.md).

## Build

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

Generate Kotlin bindings and package the native libraries, with JDK 17 in `JAVA_HOME`:

```bash
cargo xtask regen sdk
```

This runs `boltffi pack android` in `sdk/native`, configured by [`native/boltffi.toml`](native/boltffi.toml). Outputs are build products, not sources:

- `sdk/generated/`: Kotlin and JNI sources
- `sdk/jniLibs/<abi>/libreseam-sdk-native.so`: the four Android ABIs
- `sdk/dist/android/desktopJniLibs/<host>/`: the desktop JNI library, for the current host only

## Publishing

The Kotlin packages are built by the Gradle project at the workspace root:

```bash
./gradlew publishToMavenLocal -PreseamSdkVersion=0.5.0
```

- `app.reseam:reseam-sdk` for managers (Kotlin Multiplatform, Android and JVM)
- `app.reseam:reseam-patch-sdk` for patch authors

`reseam-sdk` depends on `reseam-patch-sdk`. Bundles then resolve the host's copy of the patch runtime, whose native calls reach the SDK library the host already loaded.

CI publishes both to the Reseam Maven registry on every `v*` tag, with the version taken from the tag.

## Patcher Host Requirement

Android hosts must install a classloader before inspecting or patching bundles:

```kotlin
ReseamAndroidHost.setClassLoader(classLoader)
```

The classloader must be able to resolve the Reseam SDK and patch classes. Desktop hosts need nothing: the engine attaches to the JVM it was loaded into.

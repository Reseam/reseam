# Reseam SDK

The Rust service in this directory is used directly by the CLI. Shared public data lives in `crates/model`; `sdk/native` exposes it through BoltFFI 0.30.1 for Android and desktop JVM clients.

The generated Kotlin API provides `inspect(InspectRequest)`, `patch(PatchRequest, callback)`, typed `SdkError` exceptions, and a closeable `ApkInspection`. Requests, results, events, options, and icons use generated types. No JSON transport or duplicate Kotlin DTO schema is needed. Trust is explicit: an empty `Trust.keys` trusts no bundle.

Calls are synchronous; clients choose their executor. Progress callbacks run on the calling thread and must not re-enter the engine. Keep an `ApkInspection` open while using extracted component paths.

## Generate and package

From the workspace root, install the pinned CLI and configure JDK 17, the Android SDK/NDK, Rust's four Android targets, and a host `clang` on `PATH`:

```bash
cargo install boltffi_cli --version "=$(cat .boltffi-version)" --locked
cargo xtask regen all
```

`regen patch-api` bootstraps the embedded patch bridge. `regen sdk` then runs BoltFFI's Android packer with `--deny-skipped`, including its desktop packer. The configuration is [native/boltffi.toml](native/boltffi.toml).

Outputs are disposable build products:

- `sdk/generated`: Kotlin and JNI sources.
- `sdk/jniLibs/<abi>/libreseam-sdk-native.so`: four Android ABIs.
- `sdk/dist/android/desktopJniLibs/<host>/`: desktop JNI library for the build host.

There is no separate desktop-linking task. Cross-host desktop publishing requires building the corresponding host artifact.

## Kotlin artifacts

The root Gradle build publishes `app.reseam:reseam-sdk` for applications and `app.reseam:reseam-patch-sdk` for patch authors. The application SDK shares generated JVM-compatible Kotlin between Android and desktop. It uses the Android KMP library plugin; it does not claim support for Kotlin/Native.

```bash
./gradlew publishToMavenLocal -PreseamSdkVersion=0.9.0
```

Android hosts must install their application classloader before loading bundles:

```kotlin
ReseamAndroidHost.setClassLoader(classLoader)
```

Desktop hosts attach to the app's existing JVM. BoltFFI handles native library loading and foreign-object cleanup.

For persisted selection and patch metadata, use the generated `encodeSelection` / `decodeSelection` and `encodePatchMetadata` / `decodePatchMetadata` functions. These preserve the Rust serde schema independently of the transient FFI ABI. They are not required for inspect or patch calls.

See [the architecture and migration review](../docs/bindings.md) for the two binding roots, remaining host policy, and upstream limitations.

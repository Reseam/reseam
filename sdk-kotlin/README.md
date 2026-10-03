<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">Reseam SDK for Kotlin</h1>

The `app.reseam:reseam-sdk` package for Android and JVM apps. It wraps the [Reseam SDK](../sdk/) in Kotlin, so an app can inspect APKs, load patch bundles, and patch. Reseam Manager is built on it.

The package holds the bindings generated from `sdk/native`, the native libraries, and the patch runtime that bundles run against. The calls and their rules (trust, threads, lifetimes) are in the [SDK README](../sdk/README.md#api).

## Platforms

- **Android**: API 24 and later. The library loads `libreseam-sdk-native.so` from the APK. Call `ReseamAndroidHost.setClassLoader(classLoader)` before loading bundles.
- **Desktop**: a 64-bit JVM, Java 17 or newer, on Linux x86-64 or Windows x86-64. The JVM artifact carries both native libraries and extracts the right one to `java.io.tmpdir` on first use. No `java.library.path` setup is needed.

## Build

```sh
cargo xtask regen patch-api
cargo xtask regen sdk
cargo xtask runtime
cargo xtask pack-sdk
```

`regen sdk` only needs the Rust toolchain, the pinned BoltFFI CLI, and Java. `pack-sdk` also needs the Android NDK, the Android and Windows Rust targets, llvm-mingw, and a Windows JDK; see [native build setup](../sdk/README.md#build).

To check the Kotlin side without packing native libraries:

```sh
./gradlew :reseam-sdk:compileKotlinJvm :reseam-patch-sdk:checkKotlinAbi
```

Generated Kotlin and JNI files land in `sdk/generated/`. They are build output; don't edit them.

# Host SDK bindings

The Android and JVM SDK packages the application bridge generated from `sdk/native` and the shared patch runtime. Calls are synchronous; progress callbacks run on the calling thread and must not re-enter the engine. Android hosts install their class loader with `ReseamAndroidHost.setClassLoader` before loading bundles. Close `ApkInspection` only after finishing with its component paths.

Source generation is separate from native packaging:

```sh
cargo xtask regen patch-api
cargo xtask regen sdk
cargo xtask runtime
cargo xtask pack-sdk
```

`regen sdk` requires the host Rust toolchain, the pinned BoltFFI CLI, and Java. It formats only generated SDK Kotlin. `pack-sdk` additionally requires the Android NDK, Android and Windows Rust targets, llvm-mingw, and a Windows JDK configured with `JAVA_HOME_x86_64_pc_windows_gnullvm`. See [native build configuration](../sdk/README.md#build). The runtime jar must exist before building the desktop CLI or native SDK. It is also rebuilt when patch runtime sources change.

The published JVM artifact includes Linux x86-64 and Windows x86-64 JNI libraries,
regardless of the Gradle build host. BoltFFI's generated bundled loader selects
`native/linux-x86_64/libreseam_sdk_native_jni.so` or
`native/windows-x86_64/reseam_sdk_native_jni.dll` from the runtime's OS and
architecture, extracts it to `java.io.tmpdir`, and loads it. Use a 64-bit JVM
(Java 17 or newer); no native-library installation is needed. Android continues
to load `libreseam-sdk-native.so` from the APK's ABI directory.

For host-only Kotlin validation, run `:reseam-sdk:compileKotlinJvm` and `:reseam-patch-sdk:checkKotlinAbi`. The multiplatform application module does not define `compileKotlin` or its own frozen ABI task. Generated Kotlin and JNI files are build products under `sdk/generated`.

`PreparedInspection(request)` keeps an inspection's opened APK and verified catalogs for one patch run: show `metadata()`, then call `patch(request, onEvent)` with the same inputs in the same order. Trust and payload hashes are checked again before code loads. Inputs must not change until it is consumed, and `patch` consumes it even when it fails.

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

Kotlin-visible updates from the model rewrite and regenerated bindings:

- `ProgressEvent.PatchStarted`, `.PatchLog`, and `.PatchFinished` become `.Started`, `.Log`, and `.Finished`. The application callback still uses `RunEvent.PatchStarted`, `.PatchLog`, and `.PatchFinished`.
- `JvmHeapStats.usedBytes`, `.committedBytes`, and `.maxBytes` become `.used`, `.committed`, and `.max`; all remain unsigned byte counts.
- `Problem` adds `OptionType`, `OptionChoice`, `SingleFileComponents`, `MissingPackage`, `IncompatiblePackage`, and `UnknownPreset`. Hosts with exhaustive `when` expressions must handle them.
- `ApkInspection.basePath()` and `splitPaths()` keep their Kotlin names and return types, and can now throw the generated SDK error after an interrupted inspection, like its metadata and icon methods. Close and reopen an inspection after such a failure.
- Container metadata and icon failures identify the original input instead of an extracted temporary APK. Output-writing failures use `Problem.Other`, rather than `Problem.UnreadableApk`.
- Artifact destinations must be distinct and must not overwrite signing files. A signing or publication failure preserves previous outputs; a failed rollback reports the recovery directory containing retained files.
- Metrics keep all existing fields. Phase RSS is sampled at phase end; `peakRssBytes` is the process high-water mark at that point. Allocator heap fields are null. Apply diagnostics are endpoint observations. Windows now reports current and peak working-set sizes, and Android reads its own `/proc` status where available.

Successful APK bytes and the patch authoring ABI are unchanged by these host changes. The preceding patcher rewrite moved public author value classes to `app.reseam.patch.types`; bundles built against the former concrete `native` classes must be rebuilt with the current author SDK.

Two audited generator limitations remain in pinned BoltFFI 0.31: metadata builds have no exported-root selector, and the JNI contract exposes parameter/return shapes without a complete JVM descriptor or registration-table emitter. Consequently `sdk/native` still separates application exports from the patcher dependency, and xtask retains hosted reachability and JNI registration adapters. Removing either requires upstream generator support; changing the crate topology or guessing a different ABI would break existing native identities.

`PreparedInspection(request)` keeps an inspection's opened APK and verified catalogs for one patch run: show `metadata()`, then call `patch(request, onEvent)` with the same inputs in the same order. Trust and payload hashes are checked again before code loads. Inputs must not change until it is consumed, and `patch` consumes it even when it fails.

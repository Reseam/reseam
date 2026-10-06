<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">Reseam SDK</h1>

The entry point for apps that patch. Reseam Manager, the `reseam` CLI, and the browser patcher all call it. It opens an app, reads patch bundles, checks their signers, runs the patches, and writes a signed APK.

This crate is the Rust service. [`sdk/native`](native/) exports it to Kotlin through BoltFFI, and [`sdk-kotlin/`](../sdk-kotlin/) publishes that as `app.reseam:reseam-sdk`. [`sdk/browser`](browser/) builds it for WebAssembly, used by [`@reseam/browser`](../browser/).

## API

The types come from [`crates/model`](../crates/model/).

| Call | What it does |
|---|---|
| `inspect(InspectRequest)` | Reads an APK and each bundle's signed patch list. Doesn't load patch code. |
| `PreparedInspection(request)` | Same as `inspect`, but keeps the opened APK and bundles for one `patch` call. |
| `patch(PatchRequest, onEvent)` | Runs the patches and writes the signed output. Progress arrives as `RunEvent`s. |
| `ApkInspection(apkPath, splitPaths)` | Opens an APK, APKM, or XAPK and exposes its metadata, component files, and icon. |
| `encodeSelection`, `encodePatchMetadata` | Store a selection or patch list as JSON, with matching `decode` functions. |

- **Trust.** A request lists the bundle signers it accepts in `trust.keys`. The SDK trusts no one on its own. `inspect` reads bundles from any signer and marks each one `trusted` or not; `patch` refuses untrusted ones.
- **Mount builds.** `installMethod = MOUNT` builds output to mount over the installed app. Patches that edit the manifest finish `Unmountable`; when a run finds one, the SDK sends `RunEvent.Restarted` and patches again from the original input without it.
- **Errors.** Failures are `SdkError` with a typed `Problem`, such as `UntrustedBundle`, `EngineTooOld`, or `PatchesFailed`.
- **Threads.** Calls are synchronous; run them off the main thread. `onEvent` runs on the calling thread and must not call back into the SDK.
- **Lifetimes.** `ApkInspection` component paths are valid until it is closed. A `PreparedInspection` needs the same input paths, in the same order, when you call `patch`, and the inputs must not change before then. `patch` consumes it, even on failure.

Android apps must install a class loader that can see the SDK and patch classes before loading bundles:

```kotlin
ReseamAndroidHost.setClassLoader(classLoader)
```

Desktop apps need nothing: the engine attaches to the JVM it was loaded into.

How the BoltFFI bindings are set up is in [`docs/internals/boltffi.md`](../docs/internals/boltffi.md).

## Build

Install the release Rust targets:

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  x86_64-linux-android \
  i686-linux-android \
  x86_64-pc-windows-gnullvm
```

Set the Android NDK toolchain on `PATH`. Adjust the NDK version if needed:

```bash
export ANDROID_NDK_BIN="$ANDROID_HOME/ndk/29.0.14206865/toolchains/llvm/prebuilt/linux-x86_64/bin"
export PATH="$ANDROID_NDK_BIN:$PATH"
```

Desktop releases support Linux x86-64 and Windows x86-64. On the Linux release
builder, put llvm-mingw's `bin` directory on `PATH` and extract a Windows Temurin
JDK. The target JDK supplies headers; it is never executed on Linux. Keep the
Linux JDK in `JAVA_HOME`:

```bash
export JAVA_HOME=/usr/lib/jvm/java-21-temurin-jdk
export JAVA_HOME_x86_64_pc_windows_gnullvm=/path/to/windows-jdk
export CC_x86_64_pc_windows_gnullvm=x86_64-w64-mingw32-clang
export AR_x86_64_pc_windows_gnullvm=llvm-ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER=x86_64-w64-mingw32-clang
cargo xtask regen patch-api
cargo xtask regen sdk
cargo xtask runtime
cargo xtask pack-sdk
```

JNI header selection uses `JAVA_HOME_<target>` first, then
`JAVA_HOME_<target_with_underscores>` (hyphens and dots replaced by underscores),
then `JAVA_HOME`. An explicit path must contain `include/jni.h` and the target's
`include/linux/jni_md.h`, `include/win32/jni_md.h`, or `include/darwin/jni_md.h`.
An invalid override fails instead of falling back to host headers. Native host
builds still need only `JAVA_HOME`; Android uses the NDK's headers.

`regen sdk` generates sources only. `pack-sdk` uses the pinned BoltFFI packager
for the four Android ABIs and source generation, then Cargo and the target C
compilers for Linux x86-64 and Windows x86-64 in sequence. xtask owns desktop
linking because BoltFFI 0.31 rejects Linux-to-Windows JVM packaging. System
libraries come from Rust's `native-static-libs` output; JNI headers use the same
target JDK selection as the patch bridge.
Outputs are build products, not sources:

- `sdk/generated/`: Kotlin and JNI sources
- `sdk/jniLibs/<abi>/libreseam-sdk-native.so`: the four Android ABIs
- `sdk/dist/android/desktopJniLibs/linux-x86_64/libreseam_sdk_native_jni.so`
- `sdk/dist/android/desktopJniLibs/windows-x86_64/reseam_sdk_native_jni.dll`

The JVM artifact carries both desktop libraries under `native/<platform>/`.
Generated bindings select the library by runtime OS and architecture and extract
it into the JVM's temporary directory before loading it. No `java.library.path`
setup is needed. Desktop hosts need a 64-bit JVM, Java 17 or newer. The Windows target configuration statically links the llvm-mingw runtime, so hosts do not need `libunwind.dll` or a compiler installation. Other desktop
platforms are not shipped in the release. Gradle refuses to package a JVM artifact
if either release library is missing.

## Publishing

The Kotlin packages are built by the Gradle project at the workspace root:

```bash
./gradlew publishToMavenLocal -PreseamSdkVersion=<version>
```

- `app.reseam:reseam-sdk` for apps that patch, such as Reseam Manager (Kotlin Multiplatform, Android and JVM)
- `app.reseam:reseam-patch-sdk` for patch authors

`reseam-sdk` depends on `reseam-patch-sdk`. Bundles then resolve the host's copy of the patch runtime, whose native calls reach the SDK library the host already loaded.

CI publishes both to the Reseam Maven registry on every `v*` tag, with the version taken from the tag.

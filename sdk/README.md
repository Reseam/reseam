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

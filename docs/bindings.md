# BoltFFI integration

Reseam uses BoltFFI for two Kotlin surfaces: the application SDK that Manager calls, and the patch API that bundles call from inside the engine's JVM. The CLI version in `.boltffi-version` and the Rust `boltffi*` crates move together.

## Ownership

| Component | Owns |
| --- | --- |
| `crates/model` | Shared records, enums, option values, requests, results, events, and errors |
| `crates/patcher` | Patch execution, the `#[export]` patch API, and per-bundle native registration |
| `sdk` | The application service: trust, input lifetime, output, signing, profiling |
| `sdk/native` | The `#[export]` application surface over `sdk` |
| `xtask/src/patch_api.rs` | Patch bridge generation from Binding IR |
| `sdk-kotlin` | Packaging generated Kotlin and BoltFFI-built native libraries for Android and JVM |

`sdk/native` is a separate crate because BoltFFI folds the exports of every direct dependency into a binding root. `sdk` depends on the patcher, so exporting from `sdk` would pull the patch API into the application SDK.

## Application SDK

- `boltffi pack android` builds the SDK for the four Android ABIs and the build host, compiles and links its JNI glue, and writes the Kotlin.
- Records and enums in `crates/model` are the wire schema. Manager uses the generated Kotlin types directly.
- `Result<T, SdkError>` becomes a Kotlin exception carrying a `Problem`.
- The `patch` closure parameter becomes a Kotlin callback for `RunEvent`.
- `ApkInspection` is a BoltFFI class, so extracted component files live until Kotlin closes it.
- Persisted selections and patch metadata go through `encodeSelection`, `encodePatchMetadata`, and their decoders, which serialize with serde. BoltFFI's wire bytes are version-specific and never stored.

Calls are synchronous. The engine keeps a thread-local patch context, and Manager runs calls on `Dispatchers.IO`.

## Patch bridge

Bundles call the engine through `app.reseam.patch.native`, the generated Kotlin the patch API wraps. Each bundle jar carries its own copy of the patch runtime.

The engine loads a bundle in a class loader whose parent is the host's: the app's loader on Android, the system loader on the JVM. When the host ships `reseam-patch-sdk`, as Manager does through `reseam-sdk`, every bundle resolves the host's `Native` class, and the generated loader's `System.loadLibrary` finds the SDK library already loaded. When it does not, as in the CLI, the bundle's own copy is used and the generated loader does nothing. The engine registers the bridge on whichever `Native` class the bundle resolved.

BoltFFI generates JNI entry points as exported symbols for the JVM to resolve by name. That fails for a class loaded outside the library's loader, and for symbols inside the CLI executable. `xtask/src/patch_api.rs` therefore derives a `RegisterNatives` table from the same Binding IR contract that produced the C glue. It also narrows the IR to declarations reachable from the patcher's exports, since dependency crates carry application models the patch API never sees, and configures the generated loader to load the SDK library on Android and nothing on desktop.

The patcher is a second binding root. BoltFFI emits exported functions only for the crate it generates bindings for, so `crates/patcher/build.rs` sets the expansion environment for its own crate in every build except BoltFFI's metadata builds. Without it, the SDK library and the CLI would link a patch bridge that references missing symbols.

The generated sources are not committed. `patch-api/build.gradle.kts` compiles `patch-api/generated/app` alongside the handwritten API, and `crates/patcher/build.rs` compiles `patch-api/generated/jni/registration.c`, which includes the unmodified glue.

## Known BoltFFI limitations

As of the pinned version:

- Kotlin reads direct (all-scalar) records packed on the wire, while the Rust runtime writes their padded C layout. A record with padding inside an enum payload or a vector desyncs the stream. `RegLiteralInsn`, `Branch0Insn`, `Branch2Insn`, and `TryItem` declare `#[repr(Rust)]` so BoltFFI encodes them field by field on both sides. TypeScript, Python, and Java already honour the layout; the fix belongs in the Kotlin backend.
- The experimental Kotlin Multiplatform backend rejects a record body in this surface ("KMP declaration body emission has not been ported"). The application SDK uses the JVM backend in Android/JVM shared sources instead.
- A payload variant named `String` shadows `kotlin.String` in generated code. The option variants are `Text` and `TextList` in Rust and Kotlin; their serde names stay `string` and `string_list`.
- There is no option to emit a `RegisterNatives` table, no per-dependency opt-out from export aggregation, and no way to keep generated top-level functions `internal`. The patch bridge lives in its own Kotlin package so the raw calls stay out of `app.reseam.patch`.

## Regeneration

`cargo xtask regen patch-api` needs the pinned BoltFFI CLI and `JAVA_HOME`. `cargo xtask regen sdk` also needs the Android NDK, Rust's Android targets, and the NDK's clang on `PATH`. Run `regen all` after changing an `#[export]` or a type in `crates/model`.

Bundles, the SDK library, and Manager share one FFI ABI per engine version. Rebuild bundles and upgrade Manager together.

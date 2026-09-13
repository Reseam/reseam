# BoltFFI integration

Reseam 0.9.0 uses BoltFFI 0.30.1 for both the embedded Kotlin patch API and the application SDK. The CLI pin and Rust runtime, binding, backend, and bindgen dependencies move together. The previous integration combined CLI 0.24.1 with Rust runtime 0.2.0.

## Ownership

| Component | Owns |
| --- | --- |
| `crates/model` | Shared records, enums, option values, requests, results, events, diagnostics, and errors |
| `crates/patcher` | Patch execution, hosted Kotlin calls, and per-bundle classloader registration |
| `sdk` | Application service, trust validation, input lifetime, output selection, signing, and profiling |
| `sdk/native` | Typed foreign entry points and the closeable inspection object |
| `xtask/src/patch_api.rs` | Binding IR selection and embedded host policy |
| `sdk-kotlin` | Packaging generated Kotlin and BoltFFI-built native artifacts for Android/JVM |

The dependency direction matters. Model data does not depend on the engine. The native application facade depends on the service and shared model; it does not directly export the patcher's implementation modules. Rust consumers use the service without going through FFI.

## Features used

- Binding IR replaces parsing generated C prototypes and building a second handwritten JNI signature model. One resolved contract produces patch Kotlin, the C bridge/header, and its registration table.
- Dependency closure selects only declarations reachable from hosted patch calls. Unrelated application models do not leak into the patch-author artifact.
- Generated records and payload enums replace Manager's copied JSON DTOs. Options, icon layers, metadata, progress, results, and metrics cross the boundary as typed data.
- `Result<T, SdkError>` produces typed Kotlin exceptions with a structured `Problem` and diagnostic message.
- A generated closure callback carries `RunEvent`, replacing the handwritten JSON event-sink interface.
- Foreign object ownership keeps extracted APK components alive until `ApkInspection.close()`.
- Generated unsigned arrays carry Rust register/index values. Handwritten signed-array conversion loops were removed.
- Supported default annotations preserve optional arguments and boolean defaults. Collections remain explicit because 0.30.1's default expressions do not support empty collections.
- BoltFFI's Android packer builds all four Android ABIs and its desktop packer links the host JNI library. Reseam no longer maintains SDK header generation, desktop linking, or NDK command construction.

Application calls are deliberately synchronous. The patch engine uses a thread-local context; Manager chooses `Dispatchers.IO`, and callbacks execute on that worker. Introducing async exports, streams, or another executor would add lifecycle machinery without improving this contract.

Serde remains the schema for persisted selections and patch metadata. Small exported codecs let Manager use those generated types in navigation and its bundle library without recreating field serializers. JSON is no longer the inspect/patch/event transport. Option value names in saved JSON remain `string` and `string_list`, although the Rust/Kotlin variants are now `Text` and `TextList`.

## The two binding roots

A patch bundle contains its own `app.reseam.patch.Native` class. The engine must register methods on that exact class, not a class resolved from the app's loader.

The patcher is therefore an independent native binding root even when linked into the application SDK or CLI. BoltFFI normally suppresses function exports in dependency crates. Its build script explicitly sets the published binding-expansion environment contract for this root during runtime builds; metadata builds leave that policy off. Without this, a shared-library build can appear successful while retaining unresolved hosted patch symbols.

Regeneration reads the patcher's rlib with `BindingMetadataBuild`. It selects the reachable surface and uses complete rendering, so an unsupported declaration fails generation. The registration table uses the backend's resolved JNI parameter/return types and symbols. It rejects callback/stream exports because the embedded host does not implement that separate lifecycle.

The only generated Kotlin policy adaptation removes BoltFFI's library-loader initializer and makes top-level transport calls internal. In 0.30.1, `desktop_loader = "none"` still emits Android library loading. Embedded bundles must leave loading to their host. The initializer match is exact and regeneration fails if upstream changes it. Wire codecs and C glue are untouched.

Android also retains a small host classloader setter. BoltFFI manages foreign calls; Reseam still owns loading trusted patch bundles.

## Reviewed limitations

The [Binding IR architecture](https://github.com/boltffi/boltffi/blob/v0.30.1/adrs/0001-adr-binding-ir-architecture.md), [0.30.1 configuration](https://github.com/boltffi/boltffi/blob/v0.30.1/BOLTFFI_TOML_SPEC.md), and [KMP roadmap](https://github.com/boltffi/boltffi/blob/v0.30.1/KMP_SUPPORT_ROADMAP.md) informed this integration.

The experimental Kotlin Multiplatform backend was evaluated against the actual application surface with strict coverage. It rejected a record body: “KMP declaration body emission has not been ported for record patch::result”. The application therefore uses the supported Kotlin/JNI backend in Android/JVM shared sources. It does not silently skip records or maintain a replacement KMP emitter.

A payload variant named `String` shadows Kotlin's string type in generated declarations. Naming the domain variants `Text`/`TextList` avoids rewriting generated type references.

The patcher uses the metadata library API because the CLI's library discovery expects staticlib/cdylib; the engine is intentionally an rlib. The application facade supplies the staticlib/cdylib outputs required by the official packers.

## Maintenance and validation

Run `cargo xtask regen all` after changing exported calls or shared models. Commit the published patch Kotlin source, not intermediate JNI or SDK binaries. Generation pins 0.30.1 and requires complete declaration coverage.

Build native and Kotlin artifacts together: the 0.9.0 FFI ABI is not compatible with older bundles or SDK binaries. Rebuild patch bundles and upgrade Manager together. The root, Manager, and patches Gradle builds align Kotlin 2.4.10, Gradle 9.5.1, and (where used) AGP 9.1.1 for composite development. The SDK uses Android's supported KMP library plugin. Patch dexing resolves D8 9.4.17 through Gradle, satisfying the [minimum version for Kotlin 2.4](https://developer.android.com/build/kotlin-support), instead of using whichever D8 happens to be installed in the SDK.

The migration is checked through Rust compilation/linking, Android and desktop native packaging, Manager Android/JVM compilation, and building the real patches bundle. Existing tests are not run as requested. No device or interactive UI execution is implied by these build checks.

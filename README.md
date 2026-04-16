# Reseam

A high-performance APK patching engine in Rust. Replaces the ReVanced toolchain (Java/Kotlin) with native code for DEX parsing, mutation, signing, and patch execution. Patches are written in Kotlin using a typed DSL and executed via an embedded JVM — no smali text parsing, no runtime dexlib dependency.

## Workspace

| Crate | Purpose |
|-------|---------|
| [`reseam-dex`](crates/dex/) | DEX parser, writer, and mutator for the Dalvik Executable sections Reseam currently supports |
| [`reseam-apk`](crates/apk/) | APK container: ZIP handling, Android Binary XML (AXML), resource tables, DEX extraction |
| [`reseam-sign`](crates/sign/) | APK Signature Scheme v2 signing and ECDSA P-256 key generation |
| [`reseam-patcher`](crates/patcher/) | Patch engine — bundle loading, dependency resolution, execution, Kotlin JNI host |
| [`reseam-patcher-macros`](crates/patcher-macros/) | `#[reseam_patch]` proc macro for Rust-native patches |
| [`reseam-cli`](crates/cli/) | `reseam` binary — `patch`, `list`, `info` commands |
| [`kotlin-sdk`](kotlin-sdk/) | Kotlin patch SDK — DSL, fingerprints, instruction builder, manifest/resource scopes |

### Dependency graph

```
reseam-cli
├── reseam-patcher
│   ├── reseam-apk
│   │   └── reseam-dex
│   ├── reseam-patcher-macros (proc macro)
│   ├── boltffi (optional, feature = "kotlin")
│   └── jni (optional, feature = "kotlin")
├── reseam-sign
└── reseam-apk
```

## Building

### Rust crates

```bash
cargo build                           # all crates (debug)
cargo build --release                 # all crates (release)
cargo build -p reseam-patcher         # patcher cdylib only
```

`cargo build -p reseam-patcher` produces `target/debug/libreseam_patcher.so`, which exports `boltffi_*` C-ABI symbols used by the JNI layer.

### JNI wrapper library

```bash
./kotlin-sdk/regenerate.sh
cargo build -p reseam-patcher
JAVA_HOME=/usr/lib/jvm/java-21-temurin-jdk kotlin-sdk/build-jni.sh
```

Compiles `jni_glue.c` and links against the cdylib → `target/debug/libreseam_patcher_jni.so` (exports `Java_*` JNI symbols).

### Kotlin SDK

```bash
cd kotlin-sdk && ./gradlew build
```

Requires Kotlin 1.9.25, JVM 17. Publishes as `dev.reseam:reseam-patch-sdk:0.1.0` via `maven-publish`.
The supported SDK maintenance workflow is documented in [`kotlin-sdk/README.md`](kotlin-sdk/README.md).

### Full build (all three layers)

```bash
./kotlin-sdk/regenerate.sh
cargo build -p reseam-patcher
JAVA_HOME=/usr/lib/jvm/java-21-temurin-jdk kotlin-sdk/build-jni.sh
cd kotlin-sdk && ./gradlew build
```

## Usage

### Patch an APK

```bash
reseam patch app.apk --bundle patches/ --output patched.apk
```

For split APKs:

```bash
reseam patch base.apk \
  --split config.arm64_v8a.apk \
  --split config.xxhdpi.apk \
  --bundle patches/ \
  --output-dir patched/
```

If `--key`/`--cert` are omitted, Reseam reuses or generates key material next to the output:

- single APK output: `<output>.pk8` and `<output>.der`
- split APK output directory: `<output-dir>/reseam.pk8` and `<output-dir>/reseam.der`

Use `--enable`/`--disable` to toggle patches, `--option PATCH.KEY=VALUE` to configure them, and `--dry-run` to resolve and validate without applying patches.

### CLI arguments

`reseam patch`:

- `<apk>` — base APK path
- `--bundle <PATH>` — patch bundle to load
- `--split <APK>` — repeatable split APK input
- `--output <FILE>` — output file for single-APK mode
- `--output-dir <DIR>` — output directory for split-APK mode
- `--key <PK8>` — PKCS#8 signing key path
- `--cert <DER>` — X.509 certificate path
- `--enable <PATCH>` — repeatable patch enable override
- `--disable <PATCH>` — repeatable patch disable override
- `--option PATCH.KEY=VALUE` — repeatable patch option assignment
- `--dry-run` — validate only; do not execute patches, mutate APK state, write outputs, or sign

`reseam list`:

- `--bundle <PATH>` — patch bundle to inspect

`reseam info`:

- `<apk>` — APK path to inspect

`reseam bundle keygen`:

- `--out <PATH>` — output path for the Ed25519 bundle signing seed

`reseam bundle pack`:

- `<dir>` — bundle staging directory
- `--key <PATH>` — Ed25519 private seed path
- `--out <PATH>` — output `.reseam` bundle path

### List patches in a bundle

```bash
reseam list patches/
```

### Inspect an APK

```bash
reseam info app.apk
```

## Patch bundles

```
my-patches/
├── bundle.toml               # name, author, description
├── my_patch.jar              # auto-discovered (*.jar)
└── extensions/
    └── helper.dex            # auto-discovered (extensions/*.dex)
```

Patches are `.kt` files compiled to JARs. At runtime the engine spins up an embedded JVM, loads JARs via `URLClassLoader`, and scans for `ReseamPatch` instances. Each patch's `execute()` calls back into Rust through JNI for all DEX/manifest/resource operations.

Bundle signature verification is already enforced when a bundle is loaded. `reseam patch` and `reseam list` both fail if the bundle signing key is not trusted or the manifest signature check fails.

### Execution model

- Default: runs patches with `enabled = true` plus their dependency closure
- Explicit selection: only selected patches and their dependencies run
- Explicitly disabled patches stay skipped even if depended on
- Options are validated before execution begins

## Kotlin patch SDK

Patches use a DSL that compiles to typed instruction sequences — no smali strings.

```kotlin
package dev.reseam.patches.example

import dev.reseam.patch.*

internal val targetFingerprint = fingerprint {
    accessFlags(AccessFlags.PUBLIC or AccessFlags.FINAL)
    returnType("Z")
    parameterTypes("L")
    strings("https://www.", "android.intent.action.VIEW")
}

val examplePatch = patch(
    name = "Disable deep linking",
    description = "Prevents the app from handling deep links"
) {
    compatibleWith("com.example.app")

    execute { ctx ->
        targetFingerprint.method.addInstructions(0) {
            const4(0, 1)
            return_(0)
        }

        ctx.manifest.addPermission("android.permission.INTERNET")
    }
}
```

### SDK components

| File | Purpose |
|------|---------|
| `ReseamPatch.kt` | Patch interface — metadata plus `execute(ctx: PatchRuntime)` |
| `PatchRuntime.kt` | Runtime-rooted SDK surface — `manifest`, `resources`, `bytecode`, `files`, `options`, `log` |
| `Dsl.kt` | `patch {}` and `fingerprint {}` entry points |
| `Fingerprint.kt` | Builder for method matching by access flags, return type, parameter types, strings, opcodes, literals, custom predicates |
| `Method.kt` | Method handle — instruction read/write, register access |
| `DexClass.kt` | Class handle — method/field enumeration, superclass chain |
| `InstructionBuilder.kt` | Typed instruction DSL (`const4`, `invokeStatic`, `ifEqz`, `label`, etc.) |
| `InstructionExt.kt` | Extensions on `Instruction` — `methodRef()`, `fieldRef()`, `stringRef()`, `opcode()` |
| `MethodExt.kt` | Extensions on `Method` — `returnType`, `parameterTypes` |
| `Opcodes.kt` | Named constants for all ~230 Dalvik opcodes |
| `AccessFlags.kt` | DEX access flag constants with bitwise `or` composition |
| `ManifestScope.kt` | Manifest mutation, including component-scoped split manifest access |
| `ResourceScope.kt` | Resource table access with owner-aware split lookup and component targeting |
| `FileScope.kt` | File reads/writes/copies plus XML document opening per APK component |
| `XmlScope.kt` | Generic AXML document manipulation for advanced cases |
| `Options.kt` | Option declarations plus runtime option access via `ctx.options` |

## BoltFFI integration

The FFI boundary between Rust and Kotlin uses [BoltFFI](https://crates.io/crates/boltffi) for codegen. This replaces manual JNI boilerplate with generated code at three layers:

### Architecture (Rust → C → Kotlin)

1. **Rust cdylib** (`libreseam_patcher.so`): `#[export]` functions in `crates/patcher/src/kotlin/` generate `boltffi_*` C-ABI symbols
2. **JNI wrapper** (`libreseam_patcher_jni.so`): Generated `jni_glue.c` compiled separately via `build-jni.sh`, contains `JNIEXPORT Java_dev_reseam_patch_Native_boltffi_1*` functions
3. **Kotlin bridge** (`ReseamPatcher.kt`): `private object Native` loads `reseam_patcher_jni` via `System.loadLibrary`, public functions handle wire encoding/decoding

### File locations

| Layer | Path |
|-------|------|
| Rust exports (bytecode ops) | `crates/patcher/src/kotlin/bytecode/*.rs` |
| Rust exports (manifest) | `crates/patcher/src/kotlin/manifest.rs` |
| Rust exports (resources) | `crates/patcher/src/kotlin/resources.rs` |
| Rust exports (XML) | `crates/patcher/src/kotlin/xml.rs` |
| Rust wire types | `crates/patcher/src/kotlin/types.rs` |
| BoltFFI config | `crates/patcher/boltffi.toml` |
| JNI build script | `kotlin-sdk/build-jni.sh` |
| Generated C glue | `kotlin-sdk/generated/jni/jni_glue.c` |
| Generated Kotlin (raw) | `kotlin-sdk/generated/dev/reseam/patch/ReseamPatcher.kt` |
| Published Kotlin (post-processed) | `kotlin-sdk/src/main/kotlin/dev/reseam/patch/ReseamPatcher.kt` |
| Regeneration script | `kotlin-sdk/regenerate.sh` |
| Post-processing script | `kotlin-sdk/fix-generated.sh` |
| Handwritten SDK surface | `kotlin-sdk/src/main/kotlin/dev/reseam/patch/*.kt` except `ReseamPatcher.kt` |
| Integration test | `kotlin-sdk/src/test/kotlin/dev/reseam/patch/IntegrationTest.kt` |

### Regeneration (after Rust type/export changes)

```bash
./kotlin-sdk/regenerate.sh
```

`regenerate.sh` is the supported codegen entrypoint. It runs BoltFFI with `RESEAM_SKIP_JNI_GLUE=1` so type generation does not depend on the current JNI bridge being compilable, then runs `fix-generated.sh` to copy the sanitized Kotlin source into the published SDK tree.

### Adding a new exported function

Example: adding `file_inject`.

1. Add `#[export] fn file_inject(apk_path: String, data: Vec<u8>)` in `crates/patcher/src/kotlin/files.rs`
2. Run `./kotlin-sdk/regenerate.sh`
3. Rebuild: `cargo build -p reseam-patcher && JAVA_HOME=... kotlin-sdk/build-jni.sh`
4. Expose from the handwritten SDK: `ctx.files.write(apkPath, data)` in `FileScope.kt`

### Generated source policy

Two layers:
- `kotlin-sdk/generated/` — raw BoltFFI output, fully replaceable
- `kotlin-sdk/src/main/kotlin/dev/reseam/patch/ReseamPatcher.kt` — canonical source shipped in the SDK

Handwritten files such as `ContextGuards.kt`, `ManifestScope.kt`, `ResourceScope.kt`, `XmlScope.kt`, and `Options.kt` are the stable SDK layer. Regeneration must not be the only place where API policy or ergonomics live.

Ignored build artifacts under `kotlin-sdk/generated/` and `kotlin-sdk/dev/` are not source-review targets and should not be committed back as canonical SDK source.

Regeneration flow: `regenerate.sh` generates raw artifacts with JNI compilation disabled for that step only, then `fix-generated.sh` updates the published Kotlin source. Normal Cargo builds still compile the JNI glue.

### Known codegen bugs

| Bug | Status | Workaround |
|-----|--------|------------|
| `class` keyword in parameter names | Fixed | Renamed params to `descriptor`/`class_name` on Rust side |
| Async infrastructure emitted unconditionally | Upstream bug | `fix-generated.sh` strips it via Python regex |
| `sumOf` overload ambiguity with `Vec<Option<u16>>` | Fixed | Changed to `Vec<i32>` with `-1` sentinel |

## Design decisions

- **No app-specific code in this repo** — patches are distributed separately as bundles
- **Kotlin over WASM/Lua** — familiar language for Android patch authors, compiled by Reseam, DSL-like API
- **BoltFFI for JNI codegen** — no manual JNI boilerplate
- **Handle-based FFI** — opaque `u32` handles avoid passing complex structures across the JNI boundary
- **No smali text parsing** — instructions are structured enums, not strings. This is the core performance win over ReVanced
- **In-place sort** — `sort_for_write` handles YouTube-sized DEX (35k classes) without allocating a copy
- **Dirty-flag bypass** — only re-serializes manifest/resources.arsc if actually modified
- **ZIP pass-through** — unchanged APK entries copied raw, no recompression
- **Error-first mutation/parsing** — malformed input and invalid mutation requests are expected to return errors rather than silently producing wrong output

## License

GPL-3.0

# Reseam Patch API

Kotlin patch authoring SDK for Reseam. Author docs live in [`docs/`](../docs/README.md). This directory mixes generated transport code with handwritten API code, and the boundary is strict:

- `generated/` is raw BoltFFI output and is always replaceable
- `src/main/kotlin/app/reseam/patch/ReseamPatcher.kt` is generated bridge code post-processed from `generated/` by `cargo xtask regen patch-api`; its functions are `internal`, so patch code never reaches the engine except through the handwritten surface
- every other file under `src/main/kotlin/app/reseam/patch/` is handwritten and must remain stable across regeneration

## Layout

- `app.reseam.patch`: the author surface. `Patch.kt` declares patches, `Targets.kt`, `Query.kt`, and `Point.kt` find things, `Code.kt`, `CodeEmitter.kt`, and `Hooks.kt` emit code, `Scopes.kt` wraps the manifest, resources, files, and XML, `Options.kt` and `settings/` cover user-facing configuration, `Bindings.kt` holds the structural binding compiler.
- `app.reseam.patch.dex`: the escape hatch. `Method` and `DexClass` handles, `Opcode`, `InstructionBuilder`, and instruction accessors.

## Supported workflow

Regenerate bridge artifacts after changing Rust `#[export]` functions:

```bash
cargo xtask regen patch-api      # this directory only
cargo xtask regen all            # patch-api and sdk together (recommended)
```

This runs BoltFFI with `RESEAM_SKIP_JNI_GLUE=1` so type generation is not blocked by the current JNI bridge, then post-processes the Kotlin bridge into the publishable source tree. `regen all` also rebuilds and links the Android `jniLibs/*.so` under `sdk/`, which is what you want on a fresh clone or when those binaries have gone stale.

Build the JNI wrapper library:

```bash
JAVA_HOME=/usr/lib/jvm/java-17-temurin-jdk cargo xtask jni-host
```

Run SDK tests, from the workspace root:

```bash
./gradlew :reseam-patch-sdk:test
```

## Editing rules

- Do not hand-edit files under `generated/`
- Do not hand-edit `src/main/kotlin/app/reseam/patch/ReseamPatcher.kt`
- New engine capabilities get an `#[export]` in Rust, a regeneration, and a handwritten wrapper; the generated functions stay module-private

# Reseam Patch API

Kotlin patch authoring SDK for Reseam. This directory intentionally mixes generated transport code with handwritten DSL code, but the boundary is strict:

- `generated/` is raw BoltFFI output and is always replaceable
- `src/main/kotlin/app/reseam/patch/ReseamPatcher.kt` is generated bridge code post-processed from `generated/` by `cargo xtask regen patch-api`
- `src/main/kotlin/app/reseam/patch/*.kt` excluding `ReseamPatcher.kt` are handwritten SDK surface files and must remain stable across regeneration

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

Run SDK tests:

```bash
./gradlew test
```

## Editing rules

- Do not hand-edit files under `generated/`
- Do not hand-edit `src/main/kotlin/app/reseam/patch/ReseamPatcher.kt`
- Put guards, runtime ergonomics, and stable wrapper APIs in separate handwritten files such as `PatchRuntime.kt`, `FileScope.kt`, `ResourceScope.kt`, `ManifestScope.kt`, `XmlScope.kt`, and `Options.kt`

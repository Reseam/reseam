# Reseam Patch API

Kotlin patch authoring SDK for Reseam. Author docs live in [`docs/`](../docs/README.md). This directory mixes generated transport code with handwritten API code, and the boundary is strict:

- `generated/` is raw BoltFFI output and is always replaceable; its Kotlin is compiled into this module as `app.reseam.patch.native`, its C glue into the engine
- `src/main/kotlin/app/reseam/patch/` is handwritten and must remain stable across regeneration

## Layout

- `app.reseam.patch`: the author surface. `Patch.kt` declares patches, `Targets.kt`, `Query.kt`, and `Point.kt` find things, `Code.kt`, `CodeEmitter.kt`, and `Hooks.kt` emit code, `Scopes.kt` wraps the manifest, resources, files, and XML, `Options.kt` and `settings/` cover user-facing configuration, `Bindings.kt` holds the structural binding compiler.
- `app.reseam.patch.dex`: the escape hatch. `Method` and `DexClass` handles, `Opcode`, `InstructionBuilder`, and instruction accessors.
- `app.reseam.patch.native`: generated. Its data types, such as `Instruction` and `MethodRef`, are part of the author surface. Its functions are raw engine calls for the handwritten code only.

## Supported workflow

Regenerate bridge artifacts after changing Rust `#[export]` functions:

```bash
cargo xtask regen patch-api      # this directory only
cargo xtask regen all            # patch-api and sdk together (recommended)
```

This reads the patcher's Binding IR, keeps the declarations its exports reach, and writes the Kotlin, the C glue, and the `RegisterNatives` table the engine binds to each bundle. See [BoltFFI integration](../docs/bindings.md).

## Editing rules

- Do not hand-edit files under `generated/`
- New engine capabilities get an `#[export]` in Rust, a regeneration, and a handwritten wrapper; patch code calls the wrapper

<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">Reseam Patch API</h1>

The Kotlin API patches are written against, published as `app.reseam:reseam-patch-sdk`.

To write patches, start from the [patch bundle template](https://git.reseam.app/reseam/patches-template) and follow the [docs](https://reseam.app/docs/authoring/start/). This README is for people changing the API itself.

## Layout

| Package | Contents |
|---|---|
| `app.reseam.patch` | What patch authors use. `Patch.kt` declares patches. `Targets.kt`, `Query.kt`, and `Point.kt` find code. `Code.kt`, `CodeEmitter.kt`, and `Hooks.kt` change it. `Scopes.kt` covers the manifest, resources, files, and XML. `Options.kt` and `settings/` cover user settings. `Bindings.kt` reads obfuscated objects. |
| `app.reseam.patch.dex` | Raw bytecode access: `Method`, `DexClass`, `Opcode`, `InstructionBuilder`. |
| `app.reseam.patch.types` | Generated data types such as `Instruction` and `MethodRef`. Authors use these too. |
| `app.reseam.patch.native` | Generated calls into the engine. Only the handwritten code uses them. |

Handwritten code lives in `src/main/kotlin/app/reseam/patch/`. Everything under `generated/` is BoltFFI output: never edit it by hand.

## Adding an engine capability

1. Add an `#[export]` function in `crates/patcher/src/kotlin/`.
2. Regenerate the bindings:

   ```bash
   cargo xtask regen patch-api   # this package only
   cargo xtask regen all         # this package and the app SDK
   ```

3. Wrap the generated call in handwritten Kotlin. Patches call the wrapper, never `native` directly.

`api/reseam-patch-sdk.api` records the public API. If a change to it is intended, update it with `./gradlew :reseam-patch-sdk:updateKotlinAbi`; patch authors will see that change. How the bindings are generated is in [`docs/internals/boltffi.md`](../docs/internals/boltffi.md).

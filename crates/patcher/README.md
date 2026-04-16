# reseam-patcher

Patch execution engine. Loads patch bundles, resolves dependencies, validates options, and applies patches to APKs. The current bundle loader executes Kotlin patches from compiled JARs via an embedded JVM.

## Key capabilities

- **Patch bundles** — load patches from directories with a TOML manifest, compiled Kotlin JARs, and extension DEX
- **Dependency resolution** — topological ordering of patches with cycle detection
- **Execution engine** — applies patches in order, tracks status (applied/skipped/failed), collects logs
- **Options system** — typed patch options (string, bool, int, float, string list, path) with validation and defaults
- **Kotlin patches** — spins up a JVM via JNI to execute Kotlin-authored patches that use the Reseam SDK API. Handles bytecode manipulation, manifest editing, resource copying, and XML patching through a bridge layer
- **Patch context** — provides patches access to the APK's DEX files, manifest, and resources

## Modules

| Module | Purpose |
|--------|---------|
| `engine` | `ExecutionPlan` — patch selection, ordering, execution loop |
| `bundle` | `PatchBundle` — loads patches and metadata from a bundle directory |
| `context` | `PatchContext` — APK state passed to each patch during execution |
| `patch` | `Patch` trait — interface every patch implements |
| `options` | Option declarations, types, and validation |
| `dependency` | Topological sort and cycle detection |
| `kotlin` | JNI bridge for running Kotlin patches — type conversion, bytecode ops, manifest/resource/XML helpers |
| `log` | Structured patch logging |

## Kotlin/JNI boundary

The Kotlin integration has three separate concerns:

- Rust exports in `src/kotlin/**/*.rs` define the host API with `#[export]`
- `kotlin-sdk/generated/` contains raw BoltFFI output and is fully replaceable
- handwritten files in `kotlin-sdk/src/main/kotlin/dev/reseam/patch/` provide stable SDK ergonomics and policy

Regeneration is done through `kotlin-sdk/regenerate.sh`. Normal Cargo builds remain strict and still compile the JNI glue; regeneration uses `RESEAM_SKIP_JNI_GLUE=1` only to prevent stale generated JNI code from blocking type generation.

## Usage

```rust
use reseam_patcher::prelude::*;
use reseam_patcher::bundle::PatchBundle;

let bundle = PatchBundle::load("patches/")?;
let plan = ExecutionPlan::default();
let results = reseam_patcher::engine::apply_patches_with_plan(&mut context, &bundle.patches, &plan)?;
```

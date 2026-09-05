# reseam-patcher

Patch execution engine. Loads signed patch bundles, resolves which patches run and in what order, applies them to an APK, and reports per-patch results. Patches are written in Kotlin and run on an in-process JVM.

## Key capabilities

- **Patch bundles**: open a `.reseam` archive, verify its signature and file hashes, check it was built for this engine's version line, and load its patches and extension DEX
- **Dependency resolution**: topological ordering of patches with cycle detection
- **Execution engine**: applies patches in order, tracks status (applied, skipped, failed), reports progress to an observer
- **Options system**: typed patch options (string, bool, int, float, string list, path) with validation and defaults
- **Kotlin patches**: runs Kotlin-authored patches through the BoltFFI bridge, on a JVM the engine starts or the one it was loaded into. Bytecode manipulation, manifest editing, resource and XML changes all go through the bridge
- **Patch context**: gives patches access to the APK's DEX files, manifest, resources, and files, plus a search API over all DEX

## Modules

| Module | Purpose |
|--------|---------|
| `bundle` | `BundleArchive` opens and verifies a bundle, `load` extracts it and loads its patches, `pack` writes one |
| `engine` | `apply_patches` and `validate_patches` over a `PatchSelection` |
| `context` | `PatchContext`: the APK session, DEX search API, run log, and options |
| `patch` | `Patch` trait and `PatchSpec`, the interface every patch implements |
| `options` | Option declarations, values, and resolution against a selection |
| `kotlin` | JVM host, bundle class loader, and the `#[export]` functions the Kotlin scopes call |
| `log` | Structured patch logging |

Trust is the host's decision. The engine checks that a bundle is intact and signed by the key it carries; the host decides whether that key is acceptable before calling `load`.

## Kotlin/JNI boundary

- Rust exports in `src/kotlin/**/*.rs` define the host API with `#[export]`
- `patch-api/generated/` holds raw BoltFFI output and is fully replaceable
- handwritten files in `patch-api/src/main/kotlin/app/reseam/patch/` provide the patch-author API on top

Regenerate with `cargo xtask regen patch-api`. The integration test in `tests/` builds a Kotlin fixture bundle with Gradle and runs it through the JVM.

## Usage

```rust
use reseam_patcher::bundle::BundleArchive;
use reseam_patcher::engine::{apply_patches, PatchSelection};
use reseam_patcher::Patch;

let archive = BundleArchive::open("patches.reseam".as_ref())?;
// host checks archive.public_key against its trust list here
let bundle = archive.load()?;
let patches: Vec<&dyn Patch> = bundle.patches.iter().map(Box::as_ref).collect();
let results = apply_patches(&mut context, &patches, &PatchSelection::default(), |_| {})?;
```

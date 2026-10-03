<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">reseam-patcher</h1>

Runs patches. It opens signed patch bundles, works out which patches run and in what order, applies them to an APK, and reports a result for each one. Patches are written in Kotlin and talk to this crate through generated BoltFFI bindings.

Apps that patch should call [`reseam-sdk`](../../sdk/) instead. It wraps this crate with trust checks, output handling, and signing.

- **Bundles**: verifies a `.reseam` file's signature, payload hashes, and engine version, then loads its patches and extension DEX.
- **Planning**: checks the selection and options, adds dependencies, rejects cycles, and orders patches.
- **Running**: applies patches in order. A failed patch doesn't stop unrelated ones; patches that depend on it are skipped.
- **Patch context**: gives patches the app's DEX files, manifest, resources, and files, plus search across all DEX.

## Modules

| Module | Contents |
|---|---|
| `bundle` | `BundleArchive`: open and verify, load, pack |
| `engine` | `apply_patches` and `validate_patches` for a `PatchSelection` |
| `context` | `PatchContext`: the open APK, DEX search, logs, and options |
| `patch` | `Patch`, `PatchSpec`, and compatibility declarations |
| `options` | option declarations, values, and validation |
| `kotlin` | the JVM host, bundle class loading, and the `#[export]` functions Kotlin calls |

The engine checks that a bundle is intact and signed by the key it carries. Whether to accept that key is up to the host.

The Kotlin side of the bridge lives in [`patch-api/`](../../patch-api/). After changing an `#[export]` function, run `cargo xtask regen patch-api`. [`ARCHITECTURE.md`](ARCHITECTURE.md) describes how a run flows through the modules.

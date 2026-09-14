# Reseam

Reseam is a Rust APK patching engine. Patches are written in Kotlin against the Reseam Patch API, while APK parsing, DEX mutation, serialization, and signing run natively in Rust.

## Workspace

| Crate | Purpose |
|-------|---------|
| `reseam-dex` | DEX parser, mutation, and writer |
| `reseam-apk` | APK container handling, AXML, resources, and DEX extraction |
| `reseam-sign` | APK Signature Scheme v2 signing |
| `reseam-patcher` | Bundle loading, patch execution, and Kotlin host |
| `reseam-model` | Requests, results, events, and errors shared by the engine and its clients |
| `reseam-sdk` | Shared application-facing patch service used by clients |
| `reseam-sdk-native` | BoltFFI bindings of `reseam-sdk` for Android and JVM clients |
| `reseam-cli` | `reseam` command-line interface |
| `patch-api` | Kotlin patch-author API |
| `gradle-plugin` | Gradle plugins that build a bundle from its directory layout |
| `xtask` | Build orchestration tasks (`cargo xtask …`) |

## Prerequisites

- Rust stable, plus the Android targets for the SDK's `jniLibs`:

  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
  ```

- JDK 17 in `JAVA_HOME`.
- Android SDK in `ANDROID_HOME` with a platform and an NDK. The SDK's native libraries build with the NDK's clang, so put its `toolchains/llvm/prebuilt/<host>/bin` on `PATH`.
- The BoltFFI CLI, which generates the patch bridge and the SDK bindings. Generated sources are not committed, so every checkout needs it. Install the exact version pinned in [`.boltffi-version`](.boltffi-version), from the workspace root:

  ```bash
  cargo install boltffi_cli --version "=$(cat .boltffi-version)" --locked
  ```

Commands below use POSIX shell syntax for environment variables. In PowerShell, set them first (`$env:JAVA_HOME = "C:\\jdk-17"`) and run the command on its own line.

Windows builds the CLI and the engine with the MSVC toolchain. The SDK's desktop JNI library is built for the current host only.

## Build

```bash
cargo xtask regen patch-api
cargo build --release
```

This builds the `reseam` CLI plus the embedded patcher. `cargo xtask regen all` also generates the SDK's Kotlin and packages its Android and desktop JNI libraries; run it whenever you change a `#[export]` Rust function or a type in `reseam-model`:

```bash
cargo xtask regen all
cargo build --release
```

Regeneration checks the generator version before writing files. CI regenerates everything with the pinned version.

The Kotlin side is one Gradle build at the workspace root: `patch-api` publishes `reseam-patch-sdk` for patch authors, `gradle-plugin` publishes the `app.reseam.workspace` plugin bundles build with, `sdk-kotlin` publishes `reseam-sdk` for managers. See `sdk/README.md`.

```bash
cargo xtask regen all
./gradlew assemble
```

## Release

```bash
cargo xtask release 0.5.0
git push --follow-tags
```

One version for the engine, the SDK, and the patch API, set in `Cargo.toml` by that command. CI refuses a tag that does not match it, publishes both SDK packages, and uploads the CLI. Publish the engine artifacts before releasing consumers pinned to that version.

## CLI

Patch an APK:

```bash
reseam patch app.apk \
  --bundle build/reseam/my-bundle.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
```

Measure a real patch run:

```bash
target/release/reseam perf app.apk \
  --bundle build/reseam/my-bundle.reseam \
  --warmup 1 \
  --iterations 5
```

Inspect an APK:

```bash
reseam info app.apk
```

Manage bundles:

```bash
reseam bundle keygen --out bundle-signing.key
reseam bundle pack build/reseam/stage --key bundle-signing.key --out build/reseam/my-bundle.reseam
reseam bundle list build/reseam/my-bundle.reseam --trust <PUBLIC_KEY_HEX>
```

Publish a release index:

```bash
reseam publish patches build/reseam/my-bundle.reseam \
  --version v0.1.0 \
  --url https://example.com/releases/my-bundle-v0.1.0.reseam
```

If `--key` and `--cert` are omitted during patching, Reseam reuses or generates signing material next to the output artifact.

## Bundles

Bundles are built in their own Gradle project with `./gradlew bundle`; see `docs/`. This repository's Gradle build publishes the SDKs and has no `bundle` task.

A `.reseam` bundle is a signed archive built from:

- `manifest.toml`
- compiled patch JARs
- extension DEX files

Use `reseam bundle list` to inspect bundle contents before publishing or testing.

## Documentation

- `docs/README.md` contains the patch-author guide.
- `patch-api/README.md` covers SDK maintenance and regeneration workflow.
- `docs/bindings.md` covers how the SDK and the patch API use BoltFFI.

## License

GPL-3.0

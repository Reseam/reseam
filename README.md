# Reseam

Reseam is a Rust APK patching engine. Patches are written in Kotlin against the Reseam Patch API, while APK parsing, DEX mutation, serialization, and signing run natively in Rust.

## Workspace

| Crate | Purpose |
|-------|---------|
| `reseam-dex` | DEX parser, mutation, and writer |
| `reseam-apk` | APK container handling, AXML, resources, and DEX extraction |
| `reseam-sign` | APK Signature Scheme v2 signing |
| `reseam-patcher` | Bundle loading, patch execution, and Kotlin host |
| `reseam-sdk` | Shared application-facing patch service used by clients |
| `reseam-cli` | `reseam` command-line interface |
| `patch-api` | Kotlin patch-author API |
| `xtask` | Build orchestration tasks (`cargo xtask …`) |

## Build

```bash
cargo build --release
```

This builds the `reseam` CLI plus the embedded patcher and SDK shim. The Kotlin bindings and the SDK's Android `jniLibs/*.so` files are produced by `cargo xtask regen all` (which also runs the BoltFFI codegen). Run it first on a fresh clone, and whenever you change a `#[export]` Rust function:

```bash
cargo xtask regen all
cargo build --release
```

The Kotlin side is one Gradle build at the workspace root: `patch-api` publishes `reseam-patch-sdk` for patch authors, `sdk-kotlin` publishes `reseam-sdk` for managers. See `sdk/README.md`.

```bash
JAVA_HOME=/path/to/jdk cargo xtask jni-host
./gradlew build
```

## Release

```bash
cargo xtask release 0.4.0
git push --follow-tags
```

One version for the engine, the SDK, and the patch API, set in `Cargo.toml` by that command. CI refuses a tag that does not match it, publishes both SDK packages, and uploads the CLI. The full order across repositories is in `RELEASING.md`.

## CLI

Patch an APK:

```bash
reseam patch app.apk \
  --bundle build/bundle/my-bundle.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
```

Measure a real patch run:

```bash
target/release/reseam perf app.apk \
  --bundle build/bundle/my-bundle.reseam \
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
reseam bundle pack build/staging --key bundle-signing.key --out build/bundle/my-bundle.reseam
reseam bundle list build/bundle/my-bundle.reseam --trust <PUBLIC_KEY_HEX>
```

Publish a release index:

```bash
reseam publish patches build/bundle/my-bundle.reseam \
  --version v0.1.0 \
  --url https://example.com/releases/my-bundle-v0.1.0.reseam
```

If `--key` and `--cert` are omitted during patching, Reseam reuses or generates signing material next to the output artifact.

## Bundles

A `.reseam` bundle is a signed archive built from:

- `manifest.toml`
- compiled patch JARs
- extension DEX files

Use `reseam bundle list` to inspect bundle contents before publishing or testing.

## Documentation

- `docs/README.md` contains the patch-author guide.
- `patch-api/README.md` covers SDK maintenance and regeneration workflow.

## License

GPL-3.0

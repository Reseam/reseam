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

This builds the `reseam` CLI plus the embedded patcher and SDK shim. The SDK's Android `jniLibs/*.so` files are produced by `cargo xtask regen all` (which also runs the BoltFFI codegen) — run it first on a fresh clone, or whenever you change a `#[export]` Rust function:

```bash
cargo xtask regen all
cargo build --release
```

The Kotlin SDK side is published from `patch-api/` and `sdk/`. Build them only when you want to run their gradle tests, regenerate the JNI host glue, or publish to Maven:

```bash
cd patch-api && ./gradlew test       # SDK tests
JAVA_HOME=/path/to/jdk cargo xtask jni-host
```

`cargo xtask regen` also accepts `patch-api` and `sdk` if you want to regenerate just one side.

## CLI

Patch an APK:

```bash
reseam patch app.apk \
  --bundle build/bundle/my-bundle.reseam \
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
reseam bundle list build/bundle/my-bundle.reseam
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

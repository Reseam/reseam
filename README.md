# Reseam

Reseam is a Rust APK patching engine. Patches are written in Kotlin against the Reseam SDK, while APK parsing, DEX mutation, serialization, and signing run natively in Rust.

## Workspace

| Crate | Purpose |
|-------|---------|
| `reseam-dex` | DEX parser, mutation, and writer |
| `reseam-apk` | APK container handling, AXML, resources, and DEX extraction |
| `reseam-sign` | APK Signature Scheme v2 signing |
| `reseam-patcher` | Bundle loading, patch execution, and Kotlin host |
| `reseam-library` | Shared application-facing patch service used by clients |
| `reseam-cli` | `reseam` command-line interface |
| `kotlin-sdk` | Kotlin patch SDK |

## Build

```bash
cargo build --release
cd kotlin-sdk && ./gradlew build
```

If you are working on the generated JNI boundary:

```bash
./kotlin-sdk/regenerate.sh
cargo build -p reseam-patcher
JAVA_HOME=/path/to/jdk kotlin-sdk/build-jni.sh
```

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
- `kotlin-sdk/README.md` covers SDK maintenance and regeneration workflow.

## License

GPL-3.0

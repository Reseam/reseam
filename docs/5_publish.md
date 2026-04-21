# Publishing

## Build

```bash
export RESEAM_BUNDLE_KEY=$PWD/bundle-signing.key
./gradlew bundle
```

The `bundle` Gradle task shells out to `reseam bundle pack`. Override the CLI location with `RESEAM_BIN` if it isn't on the default path. Output: `build/bundle/<name>.reseam`.

For keygen and first-run setup, see [Setup](1_setup.md).

## Apply locally to verify

```bash
reseam bundle list build/bundle/<name>.reseam
reseam patch target.apk \
  --bundle build/bundle/<name>.reseam \
  --output patched.apk
```

For split APKs, pass each extra split with `--split`. Toggle individual patches with `--enable <name>` / `--disable <name>`; pass options with `--option <patch>.<key>=<value>`. `--dry-run` resolves the bundle without writing an APK.

## Benchmark locally

Use the release CLI for meaningful numbers:

```bash
cargo build --release -p reseam-cli
target/release/reseam perf target.apk \
  --bundle build/bundle/<name>.reseam \
  --warmup 1 \
  --iterations 5
```

`reseam perf` runs the real patch pipeline into a temporary output location and reports total duration plus per-phase duration, RSS, and peak RSS. Pass `--json` when you want to archive or compare results in tooling.

## Release index

```bash
reseam publish patches \
  build/bundle/<name>.reseam \
  --version v0.3.0 \
  --url https://example.com/releases/<name>-v0.3.0.reseam
```

Required: `--version`, `--url`. Optional: `--homepage`, `--description` or `--description-file`, `--created-at`, `--prerelease`, `--out` (defaults to `patches.json`).

If the output `patches.json` already exists, prior releases are preserved and an entry matching `--version` is replaced.

In CI, the `generatePatchesJson` Gradle task wraps this command and reads arguments from environment variables: `RESEAM_RELEASE_VERSION`, `RESEAM_BUNDLE_URL`, `RESEAM_RELEASE_DESCRIPTION` (or `RESEAM_RELEASE_DESCRIPTION_FILE`), `RESEAM_HOMEPAGE`, `RESEAM_RELEASE_CREATED_AT`, `RESEAM_RELEASE_PRERELEASE`, `RESEAM_PATCHES_JSON_OUT`.

## Hosting

Host the `.reseam` file and `patches.json` on any static host. Treat `.reseam` files as immutable: publish a new version at a new URL rather than overwriting.

Publish your public key fingerprint on an identity users already trust (project site, source repo) so they can verify the copy embedded in `patches.json`.

# Publishing

## Build

```bash
./gradlew bundle
```

Output: `build/reseam/<name>.reseam`. See [Setup](1_setup.md) for how the CLI and signing key are located.

## Apply locally

```bash
reseam bundle list build/reseam/<name>.reseam --trust <PUBLIC_KEY_HEX>
reseam patch target.apk \
  --bundle build/reseam/<name>.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
```

For split APKs, pass each extra split with `--split`, or pass an APKM or XAPK file. Toggle patches with `--enable <name>` and `--disable <name>`; pass options with `--option <patch>.<key>=<value>`. `--dry-run` resolves the bundle without writing an APK.

`bundle list` shows user-facing patches. Internal patches are counted, not listed.

## Benchmark

```bash
reseam perf target.apk \
  --bundle build/reseam/<name>.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --warmup 1 \
  --iterations 5
```

`reseam perf` runs the real pipeline into a temporary output and reports total and per-phase duration, RSS, and peak RSS. `--json` writes machine-readable results.

## Release index

```bash
reseam publish patches \
  build/reseam/<name>.reseam \
  --version v0.5.0 \
  --url https://example.com/releases/<name>-v0.5.0.reseam
```

Required: `--version`, `--url`. Optional: `--homepage`, `--description` or `--description-file`, `--created-at`, `--prerelease`, `--out` (defaults to `patches.json`). An existing `patches.json` keeps prior releases; an entry matching `--version` is replaced.

In CI, the `generatePatchesJson` Gradle task wraps this command. `-PreleaseTag=vX.Y.Z` derives the version and the official download URL; `RESEAM_RELEASE_VERSION`, `RESEAM_BUNDLE_URL`, `RESEAM_RELEASE_DESCRIPTION` or `RESEAM_RELEASE_DESCRIPTION_FILE`, `RESEAM_HOMEPAGE`, `RESEAM_RELEASE_CREATED_AT`, `RESEAM_RELEASE_PRERELEASE`, and `RESEAM_PATCHES_JSON_OUT` override each value. `stageRelease` collects the bundle and `patches.json` under `build/reseam/release/`.

## Hosting

Host the `.reseam` file and `patches.json` on any static host. Treat `.reseam` files as immutable: publish a new version at a new URL rather than overwriting.

Publish your public key on an identity users already trust, so they can verify the copy embedded in `patches.json`.

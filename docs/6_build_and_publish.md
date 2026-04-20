# Build and publish

## Build

```bash
export RESEAM_BUNDLE_KEY=$PWD/bundle-signing.key
./gradlew bundle
```

Output: `build/bundle/<bundle-name>.reseam`.

Generate a signing seed once:

```bash
reseam bundle keygen --out bundle-signing.key
```

Keep it private. Store it outside the repository.

## Verify

```bash
reseam bundle list build/bundle/<bundle-name>.reseam
```

Apply locally:

```bash
reseam patch target.apk \
  --bundle build/bundle/<bundle-name>.reseam \
  --output patched.apk
```

## Publish

Generate a release index:

```bash
reseam publish patches \
  build/bundle/<bundle-name>.reseam \
  --version v0.3.0 \
  --url https://example.com/releases/my-bundle-v0.3.0.reseam
```

- `--version` and `--url` are required.
- `--homepage`, `--description` (or `--description-file`), `--created-at`, `--prerelease` are optional.

If `patches.json` already exists, existing releases are preserved and the same version is replaced.

The Gradle task `generatePatchesJson` wraps this for CI. It reads from environment variables (`RESEAM_RELEASE_VERSION`, `RESEAM_BUNDLE_URL`, etc.).

## Hosting

Host the `.reseam` binary and the `patches.json` on any static host. Treat `.reseam` files as immutable: publish a new version under a new URL rather than overwriting.

## Key distribution

`reseam publish patches` writes your public key into `patches.json` automatically. Publish the same fingerprint on an identity users already trust (project site, source repo) so they can verify.

# reseam-cli

Command-line interface for Reseam. Provides the `reseam` binary.

The CLI trusts no bundle signer on its own. Every command that loads a bundle takes `--trust <PUBLIC_KEY_HEX>`, repeatable, naming the signers you accept; a bundle signed by anyone else is refused before any of its code runs.

## Commands

### `reseam patch`

Apply a patch bundle to an APK.

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> --output patched.apk
```

For split APKs:

```bash
reseam patch base.apk \
  --split config.arm64_v8a.apk \
  --split config.xxhdpi.apk \
  --bundle patches.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output-dir patched/
```

Options:
- `--split <APK>`: add a split APK alongside the base APK, repeatable
- `--output <FILE>`: output path for single-APK mode
- `--output-dir <DIR>`: output directory for split-APK mode
- `--key <PK8>` and `--cert <DER>`: sign with an existing PKCS#8 key and X.509 certificate, provided together; otherwise Reseam reuses or generates key material next to the output
- `--enable <PATCH>` and `--disable <PATCH>`: toggle patches by name, repeatable
- `--option PATCH.KEY=VALUE`: set a patch option, typed by the patch's declaration
- `--dry-run`: resolve and validate without applying patches or writing output

### `reseam perf`

Run the same pipeline as `patch` into a temporary output and report per-phase timings and memory. Takes the `patch` flags plus:

```bash
reseam perf app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> --warmup 1 --iterations 5 --json
```

- `--iterations <N>`: measured runs, default 1
- `--warmup <N>`: unmeasured runs first, default 0
- `--json`: machine-readable report instead of plain text

### `reseam info`

Print APK metadata: package name, version, DEX file count, and split information.

```bash
reseam info app.apk
```

### `reseam bundle keygen`

Generate an Ed25519 signing seed for bundle packing. Prints the public key; that hex string is what users pass to `--trust`.

```bash
reseam bundle keygen --out reseam.key
```

### `reseam bundle pack`

Pack a staging directory into a signed `.reseam` bundle. The directory holds `manifest.toml` with a `[bundle]` table (`name`, `format_version`, optional `author` and `description`) beside the `.jar` and `.dex` payload. Jars must carry both JVM classes and `classes.dex` so the same bundle runs on desktop and on Android. The bundle is stamped with the engine version that packed it.

```bash
reseam bundle pack staging/ --key reseam.key --out patches.reseam
```

### `reseam bundle list`

Show a bundle's metadata, signer, engine version, and, when the signer is trusted, every patch with its compatibility, dependencies, and options.

```bash
reseam bundle list patches.reseam --trust <PUBLIC_KEY_HEX>
```

### `reseam publish patches`

Add a release to a `patches.json` index, the file Reseam Manager and the Reseam API read to find bundle releases. Takes the publisher identity and public key from the signed archive, replaces any release with the same version, and refuses to change the index's signer.

```bash
reseam publish patches patches.reseam --version v0.1.0 --url https://example.com/patches-v0.1.0.reseam --description-file CHANGELOG.md
```

### `reseam publish manager`

Add a release to a `manager.json` index, the file Reseam Manager checks for updates. Same shape as `patches.json`; the publisher identity comes from flags.

```bash
reseam publish manager --name "Reseam Manager" --author Reseam --version 1.0.0 --url https://example.com/manager/releases/v1.0.0
```

Both `publish` commands accept `--out`, `--description` or `--description-file`, `--homepage`, `--created-at`, and `--prerelease`.

## Documentation

Full reference for every command lives in `docs/`.

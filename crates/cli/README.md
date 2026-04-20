# reseam-cli

Command-line interface for Reseam. Provides the `reseam` binary.

## Commands

### `reseam patch`

Apply a patch bundle to an APK.

```bash
reseam patch app.apk --bundle patches.reseam --output patched.apk
```

For split APKs:

```bash
reseam patch base.apk \
  --split config.arm64_v8a.apk \
  --split config.xxhdpi.apk \
  --bundle patches.reseam \
  --output-dir patched/
```

Options:
- `--split <APK>` — add a split APK alongside the base APK
- `--output <FILE>` — output path for single-APK mode
- `--output-dir <DIR>` — output directory for split-APK mode
- `--key` / `--cert` — sign with an existing PKCS#8 key and X.509 cert (otherwise Reseam reuses or generates sibling key material next to the output)
- `--enable` / `--disable` — toggle specific patches by name
- `--option PATCH.KEY=VALUE` — set patch options
- `--dry-run` — resolve and validate without applying patches
- bundle signatures are verified automatically when the bundle is loaded; patching fails if the signer is not trusted or the manifest signature is invalid

Argument summary:
- `<apk>` — base APK path
- `--bundle <PATH>` — patch bundle to load
- `--split <APK>` — repeatable split APK input
- `--output <FILE>` — output file for single-APK mode
- `--output-dir <DIR>` — output directory for split-APK mode
- `--key <PK8>` and `--cert <DER>` — APK signing material, provided together
- `--enable <PATCH>` / `--disable <PATCH>` — repeatable patch selection overrides
- `--option PATCH.KEY=VALUE` — repeatable patch option assignment
- `--dry-run` — validation-only mode

### `reseam info`

Print APK metadata: package name, version, DEX file count, and split/component info.

```
reseam info app.apk
```

Arguments:
- `<apk>` — APK path to inspect

### `reseam bundle list`

List all patches in a bundle with their metadata and compatibility info.

```
reseam bundle list patches.reseam
```

Arguments:
- `<bundle>` — bundle to inspect

### `reseam bundle keygen`

Generate an Ed25519 signing seed for bundle packing.

```bash
reseam bundle keygen --out reseam.key
```

Arguments:
- `--out <PATH>` — output path for the private seed

### `reseam bundle pack`

Pack and sign a bundle staging directory.

```bash
reseam bundle pack my-bundle/ --key reseam.key --out patches.reseam
```

Arguments:
- `<dir>` — bundle staging directory
- `--key <PATH>` — Ed25519 private seed path
- `--out <PATH>` — output bundle path

### `reseam publish patches`

Generate or update a `patches.json` release index from a signed `.reseam` bundle.

```bash
reseam publish patches \
  build/bundle/reseam-patches.reseam \
  --version v0.1.0 \
  --url https://reseam.app/releases/reseam-patches-v0.1.0.reseam
```

The command validates the bundle's embedded `manifest.sig`, reads `manifest.pubkey`, and writes
the public key into the index. If `patches.json` already exists, existing releases are preserved
and any release with the same version is replaced.

Arguments:
- `<bundle>` — signed `.reseam` bundle to publish
- `--version <VERSION>` — release version string
- `--url <URL>` — public download URL for the `.reseam` bundle
- `--out <PATH>` — output index path; defaults to `patches.json`
- `--description <TEXT>` — release description
- `--description-file <PATH>` — release description loaded from a file
- `--homepage <URL>` — bundle homepage; preserves the existing value when omitted
- `--created-at <ISO8601>` — release timestamp; defaults to the current UTC time
- `--prerelease` — mark the release as a prerelease

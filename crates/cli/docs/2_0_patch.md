---
title: Patch
description: Apply a signed bundle to an APK.
---

# `reseam patch`

![Diagram: the reseam patch pipeline. Inputs are the base APK (plus optional splits) and a signed .reseam bundle. The patch engine verifies the bundle signature, resolves enable/disable/options, and runs patches in order. Output is a v2-signed patched APK. A sidecar .pk8 private key and .der certificate are written next to the output (or reused from the previous run) so the same signing identity persists across invocations.](patch-pipeline.svg)

Applies patches from a signed bundle to an APK and writes a new, v2-signed APK (or split APK set).

## Single APK

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> --output patched.apk
```

Without `--output`, the CLI writes `<stem>-patched.apk` next to the input APK.

## Split APKs

```bash
reseam patch base.apk \
  --split config.arm64_v8a.apk \
  --split config.xxhdpi.apk \
  --bundle patches.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output-dir patched/
```

`--split` is repeatable. Without `--output-dir`, the CLI writes a `<stem>-patched/` directory next to the base APK and places the signed outputs inside it.

## APKM and XAPK containers

The input can also be an `.apkm` or `.xapk`. Reseam validates and extracts its APK components, then uses the same patching and signing pipeline as an explicit split set. Container inputs cannot be combined with `--split`.

Without output flags, one component produces `<stem>-patched.apk`; multiple components produce `<stem>-patched/`. `--output` requires one component. `--output-dir` works for either case and preserves the component filenames. The two flags are mutually exclusive.

XAPKs containing or declaring OBB expansion files are rejected: Reseam currently handles APK components only.

## Arguments

| Argument | Purpose |
|----------|---------|
| `<apk>` | Base APK, APKM, or XAPK path. |
| `--bundle <PATH>` | Signed `.reseam` bundle to load. Verified on open. Repeatable, since a patch may depend on one from another bundle. |
| `--trust <PUBLIC_KEY_HEX>` | Repeatable. Ed25519 public key of a bundle signer to accept. Without it no bundle loads. |
| `--split <APK>` | Repeatable split APK alongside the base. |
| `--output <FILE>` | Output path for single-APK mode. Mutually exclusive with `--output-dir`. |
| `--output-dir <DIR>` | Output directory for APK components (one or more). Mutually exclusive with `--output`. |
| `--key <PK8>` | PKCS#8 private key for APK signing. Requires `--cert`. |
| `--cert <DER>` | DER-encoded X.509 certificate matching `--key`. Requires `--key`. |
| `--preset <PRESET>` | Patches to start from: `recommended` (default), `all` or `none`. See [Selecting patches](#selecting-patches). |
| `--enable <PATCH>` | Repeatable. Add a patch to the preset, even if disabled by default. `PATCH` is `<bundle>/<id>`, an ID unique across the loaded bundles, or an unambiguous display name. |
| `--disable <PATCH>` | Repeatable. Remove a patch from the preset. |
| `--option PATCH.KEY=VALUE` | Repeatable. Set a patch option. Parsed against the patch's declared option type. |
| `--dry-run` | Resolve and validate without applying patches or writing output. |
| `--ignore-versions` | Run patches on app versions they were not declared for. The package check still applies. |

## Signing

If you pass `--key` and `--cert`, the CLI uses that PKCS#8 key and DER-encoded X.509 cert to produce the APK v2 signature. If you don't:

- Single-APK mode: Reseam looks for `<stem>.pk8` and `<stem>.der` next to the output, where `<stem>` is the output name without its extension (`patched.pk8` beside `patched.apk`). If both exist, it reuses them; if neither exists, it generates a fresh ECDSA P-256 keypair with a self-signed certificate and writes them to those paths. If only one exists, it reports an error and preserves that file.
- Split-APK mode: Reseam looks for `reseam.pk8` and `reseam.der` inside `--output-dir`. The same pair checks apply. All splits are signed with the same key.

Bundle signatures are verified on load against the bundle's embedded public key, then the CLI checks that signer against the keys passed with `--trust`. The CLI ships no keys of its own. An unsigned bundle, a bundle whose signer was not passed with `--trust`, or a tampered manifest stops the run before any patching happens.

## Dry run

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> --dry-run
```

Validates each patch against the APK's package and version and logs one line per patch. Exits non-zero if any patch fails validation. Nothing is written to disk.

## Selecting patches

The selection starts from a preset, then `--enable` adds patches and `--disable` removes them:

| Preset | Selects |
|--------|---------|
| `recommended` (default) | Patches that declare the APK's package and are enabled by default. |
| `all` | Every patch that declares the APK's package. |
| `none` | Nothing; only what `--enable` names. |

Presets never take patches for other apps, or universal patches, which declare no package. A universal patch runs only when enabled by name. A version mismatch still skips a selected patch, and a skipped patch does not pull in its dependencies.

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> \
  --enable example-patch \
  --disable other-patch
```

Run only the named patches:

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> \
  --preset none --enable example-patch
```

Set a patch option:

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> \
  --option example-patch.mode=fast
```

The value is parsed against the option's declared type: string, bool, int, float, string list, or path. An unknown patch or key fails the run before any DEX work starts.

## Output

Each patch logs a line as it finishes (applied, skipped with a reason, or failed with a reason), and a summary line records the counts. One or more failed patches exit non-zero. The patched APK (or split set) is written only after every selected patch applied cleanly.

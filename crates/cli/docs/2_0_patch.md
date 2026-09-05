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

## Arguments

| Argument | Purpose |
|----------|---------|
| `<apk>` | Base APK path. |
| `--bundle <PATH>` | Signed `.reseam` bundle to load. Verified on open. |
| `--trust <PUBLIC_KEY_HEX>` | Repeatable. Ed25519 public key of a bundle signer to accept. Without it no bundle loads. |
| `--split <APK>` | Repeatable split APK alongside the base. |
| `--output <FILE>` | Output path for single-APK mode. Mutually exclusive with `--output-dir`. |
| `--output-dir <DIR>` | Output directory for split-APK mode. Mutually exclusive with `--output`. |
| `--key <PK8>` | PKCS#8 private key for APK signing. Requires `--cert`. |
| `--cert <DER>` | DER-encoded X.509 certificate matching `--key`. Requires `--key`. |
| `--enable <PATCH>` | Repeatable. Force a patch on, even if disabled by default. |
| `--disable <PATCH>` | Repeatable. Force a patch off. |
| `--option PATCH.KEY=VALUE` | Repeatable. Set a patch option. Parsed against the patch's declared option type. |
| `--dry-run` | Resolve and validate without applying patches or writing output. |

## Signing

If you pass `--key` and `--cert`, the CLI uses that PKCS#8 key and DER-encoded X.509 cert to produce the APK v2 signature. If you don't:

- Single-APK mode: Reseam looks for `<stem>.pk8` and `<stem>.der` next to the output, where `<stem>` is the output name without its extension (`patched.pk8` beside `patched.apk`). If both exist, it reuses them; otherwise it generates a fresh ECDSA P-256 keypair with a self-signed certificate and writes them to those paths.
- Split-APK mode: Reseam looks for `reseam.pk8` and `reseam.der` inside `--output-dir`. Same reuse-or-generate behavior. All splits are signed with the same key.

Bundle signatures are verified on load against the bundle's embedded public key, then the CLI checks that signer against the keys passed with `--trust`. The CLI ships no keys of its own. An unsigned bundle, a bundle whose signer was not passed with `--trust`, or a tampered manifest stops the run before any patching happens.

## Dry run

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> --dry-run
```

Validates each patch against the APK's package and version and logs one line per patch. Exits non-zero if any patch fails validation. Nothing is written to disk.

## Selecting patches

By default, every patch in the bundle runs if its `enabled_by_default` flag is set. Override per patch:

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> \
  --enable example-patch \
  --disable other-patch
```

Set a patch option:

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> \
  --option example-patch.mode=fast
```

The value is parsed against the option's declared type: string, bool, int, float, string list, or path. An unknown patch or key fails the run before any DEX work starts.

## Output

Each patch logs a line as it finishes (applied, skipped with a reason, or failed with a reason), and a summary line records the counts. One or more failed patches exit non-zero. The patched APK (or split set) is written only after every selected patch applied cleanly.

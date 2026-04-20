---
title: Bundle
description: Generate signing keys, pack bundles, and list their contents.
---

# `reseam bundle`

Subcommands for building and inspecting `.reseam` bundles.

## `reseam bundle keygen`

Generate an Ed25519 signing seed for bundle packing.

```bash
reseam bundle keygen --out reseam.key
```

Writes a raw 32-byte seed with mode `0600`. The public key is printed as hex so clients can identify and trust the signer:

```
Ed25519 keypair generated
  private seed: reseam.key
  public key (hex): 1f3c...
  trust this signer in your client before loading its bundles
```

The command refuses to overwrite an existing file.

| Argument | Purpose |
|----------|---------|
| `--out <PATH>` | Where to write the private seed. |

## `reseam bundle pack`

Pack and sign a bundle staging directory into a `.reseam` archive.

```bash
reseam bundle pack my-bundle/ --key reseam.key --out patches.reseam
```

The staging directory must contain a `manifest.toml` with a `[bundle]` table. Required fields: `name`, `format_version`. Optional: `author`, `description`. Every `.jar`, `.dex`, and `.rve` file in the directory is packed; other files (including `manifest.toml` itself) are ignored. The pack fails if no payload files are found.

The command:

1. Parses `manifest.toml` and checks `format_version` matches `reseam_patcher::bundle::BUNDLE_FORMAT_VERSION`.
2. Reads the payload files, sorts them by name, and hashes each with SHA-256.
3. Rewrites the manifest with a `[files]` table of name-to-hex-SHA-256 pairs.
4. Derives the Ed25519 keypair from the `--key` seed and signs the rewritten manifest.
5. Writes the zip: `mimetype` (stored), `manifest.toml` (deflated), `manifest.pubkey` (stored), `manifest.sig` (stored), then each payload file (deflated).

| Argument | Purpose |
|----------|---------|
| `<dir>` | Bundle staging directory. |
| `--key <PATH>` | Ed25519 seed from `reseam bundle keygen`. |
| `--out <PATH>` | Output `.reseam` archive path. |

## `reseam bundle list`

List every patch in a bundle with its metadata.

```bash
reseam bundle list patches.reseam
```

Output shape:

```
bundle: ytm-patches
author: reseam
description: YouTube and YouTube Music patches

    1. [on] block-ads - Remove ads from the timeline.
       packages: com.google.android.youtube (19.x, 20.x)
       depends: block-ads-core
       options:
         - mode (Enum, optional)
```

The bundle is loaded through the same path `reseam patch` uses: its signature is verified against the bundle's embedded public key, then the CLI applies its trust policy for that signer. A bundle with an invalid signature, an untrusted signer, or the wrong `format_version` fails before any patch metadata is printed.

| Argument | Purpose |
|----------|---------|
| `<bundle>` | `.reseam` archive to inspect. |

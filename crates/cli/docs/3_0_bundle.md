---
title: Bundle
description: Generate signing keys, pack bundles, and list their contents.
---

# `reseam bundle`

![Diagram of the inside of a .reseam zip archive in order. First the mimetype entry (stored, uncompressed, first in the zip). Then manifest.toml (deflated) carrying the bundle table and a files table of SHA-256 hashes per payload entry. Then manifest.pubkey (stored) and manifest.sig (stored) holding the Ed25519 public key and signature over the manifest. Below a divider, the payload: every .jar and .dex file in the bundle, deflated. On load the engine verifies the signature against the public key, checks the public key against the client's trust list, then re-hashes every payload file and compares against the files table.](bundle-anatomy.svg)

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
```

The public key is what clients pass to trust the signer: `--trust` on the CLI, the trust list in Reseam Manager.

The command refuses to overwrite an existing file.

| Argument | Purpose |
|----------|---------|
| `--out <PATH>` | Where to write the private seed. |

## `reseam bundle pack`

Pack and sign a bundle staging directory into a `.reseam` archive.

```bash
reseam bundle pack my-bundle/ --key reseam.key --out patches.reseam
```

The staging directory must contain a `manifest.toml` with a `[bundle]` table. Required fields: `name`, `format_version`. Optional: `author`, `description`. The command adds `engine`, the version of the CLI doing the packing; bundles load on engines of the same major version (same minor while the major is 0). Every `.jar` and `.dex` file in the directory is packed; other files (including `manifest.toml` itself) are ignored. The pack fails if no payload files are found.

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
reseam bundle list patches.reseam --trust <PUBLIC_KEY_HEX>
```

Output shape:

```
bundle: example-bundle
author: example
description: Example patches
signer: 1f3c... (trusted)
engine: 0.3.0
files: example-patches.jar, example-extension.dex

    1. [on] example-patch - One-line description.
       packages: com.example.app (1.0.0, 1.1.0)
       depends: example-core
       options:
         - mode (String, optional)
```

`signer` is the bundle's public key and whether it matched `--trust`; `engine` is the version of the CLI that packed it; `files` lists the payload, jars and extension DEX alike. `(String, optional)` is the option's declared type and required flag.

The bundle is loaded through the same path `reseam patch` uses: its signature is verified against the bundle's embedded public key, then that key is checked against `--trust`. Without a matching `--trust` the command prints the bundle metadata and signer but not the patches, since listing them means loading the bundle's code. A bundle with an invalid signature or the wrong `format_version` fails before any metadata is printed.

| Argument | Purpose |
|----------|---------|
| `<bundle>` | `.reseam` archive to inspect. |
| `--trust <PUBLIC_KEY_HEX>` | Repeatable. Signer to accept so patches are listed. |

A bundle from an incompatible engine line fails to open with a message naming which side needs updating.

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

Writes a raw 32-byte seed, with mode `0600` on Unix. The public key is printed as hex so clients can identify and trust the signer:

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
4. Initializes the patch declarations using the runtime metadata reader and adds their complete `PatchSpec` catalog to the manifest. Packing executes the author's code and requires a JVM when patch jars are present.
5. Derives the Ed25519 keypair from the `--key` seed and signs the rewritten manifest, including the patch catalog.
6. Writes the zip: `mimetype` (stored), `manifest.toml` (deflated), `manifest.pubkey` (stored), `manifest.sig` (stored), then each payload file (deflated).

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
bundle: example-bundle
author: example
description: Example patches
signer: 1f3c... (untrusted)
files: 2

    1. [on] Example patch - One-line description.
       id: app.example.examplePatch
       packages: com.example.app (1.0.0, 1.1.0)
       depends: example-bundle/app.example.exampleCore
       options:
         - mode (String, optional)
```

`signer` is the bundle's public key and whether it matched `--trust`; `files` counts the payload files. `--verbose` adds the packing engine version and lists each payload filename on its own line. `(String, optional)` is the option's declared type and required flag. `--json` prints the full inspection response as JSON, including filenames, engine version and internal patches, which is what the Gradle plugin reads to generate references to another bundle's patches.

```bash
reseam bundle list patches.reseam --json
```

Listing reads the signed patch catalog without extracting payloads, starting a JVM, or executing bundle code. Every readable bundle's patches are listed, regardless of signer trust. `--trust` only marks whether the signer is approved for patching; it does not affect listing. Patching still checks trust before loading code and rejects loaded metadata that differs from the signed catalog.

Bundles packed before the static catalog was introduced must be rebuilt. `format_version` remains `1`; the catalog is now required. A bundle with an invalid signature, missing catalog, or unsupported format fails before any metadata is printed in the human-readable listing; JSON records bundle failures in the `problem` field. Payload hashes are checked when patching loads the bundle, rather than during catalog inspection.

| Argument | Purpose |
|----------|---------|
| `<bundle>` | `.reseam` archive to inspect. |
| `--trust <PUBLIC_KEY_HEX>` | Repeatable. Mark a signer as trusted; optional for listing. |
| `--verbose` | Include the packing engine version and individual payload filenames. |
| `--json` | Print the full inspection response as JSON. Conflicts with `--verbose`. |

A bundle from an incompatible engine line fails to open with a message naming which side needs updating.

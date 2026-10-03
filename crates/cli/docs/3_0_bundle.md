---
title: Bundle
description: Create signing keys, pack bundles, and list what is inside one.
---

# `reseam bundle`

Most authors only call these through the Gradle build. They are useful for checking a bundle by hand.

## `reseam bundle keygen`

```bash
reseam bundle keygen --out ~/.reseam/bundle-signing.key
```

Creates a new signing key and prints its public key:

```text
Ed25519 keypair generated
  private seed: /home/you/.reseam/bundle-signing.key
  public key (hex): 1f3c...
```

Users trust that public key. It never overwrites an existing file. Keep the key file private and backed up.

## `reseam bundle list`

```bash
reseam bundle list my-patches.reseam
```

Shows the bundle's name, author, signer (and whether you trust it), and every patch with its ID, description, apps, dependencies, and options. Internal patches are counted, not listed.

| Flag | |
|---|---|
| `--trust <key>` | mark this signer as trusted in the output; repeatable |
| `--verbose` | also show the engine version and the files inside |
| `--json` | print everything as JSON |

Listing reads the bundle's signed metadata and never runs its code, so `--trust` is optional. Without it, the signer shows as untrusted.

## `reseam bundle pack`

```bash
reseam bundle pack build/reseam/stage --key ~/.reseam/bundle-signing.key --out my-patches.reseam
```

Packs and signs a staging folder into a `.reseam` file. The Gradle build creates the staging folder and calls this for you. The folder holds `manifest.toml`, the patch `.jar` and extension `.dex` files, and an optional `resources/` folder.

Packing loads the patches once to record their metadata, so it needs Java.

## `reseam bundle manifest`

```bash
reseam bundle manifest manifest.toml
```

Checks a `manifest.toml` and prints its bundle details as JSON. The Gradle build uses it to read the bundle name.

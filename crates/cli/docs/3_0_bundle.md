---
title: Bundle
description: Create signing keys, pack bundles, list what is inside one, and fetch published releases.
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

## `reseam bundle fetch`

```bash
reseam bundle fetch --index https://patches.example.com/patches.json --version 1.4.0 --to bundles
```

Downloads a release named in a published index and prints the path of the bundle file. The Gradle build calls it for `bundle(index = ..., version = ...)`.

| Flag | |
|---|---|
| `--index <url>` | the release index; `file:` URLs read from disk |
| `--version <version>` | the release; a leading `v` is ignored |
| `--signer <key>` | fail unless the index names this public key |
| `--to <folder>` | where the bundle is kept, as `<name>-<version>.reseam`; defaults to the current folder |

The bundle must be signed by the key the index names, with the name and version the index lists. A file already in the folder that passes these checks is not downloaded again.

## `reseam bundle pack`

```bash
reseam bundle pack build/reseam/stage --key ~/.reseam/bundle-signing.key --out my-patches.reseam
```

Packs and signs a staging folder into a `.reseam` file. The Gradle build creates the staging folder and calls this for you. The folder holds `manifest.toml`, the patch `.jar` and extension `.dex` files, and an optional `resources/` folder.

`--version <version>` records the release in the signed manifest, where [`reseam release patches`](/docs/cli/release/) and the app's settings screen read it. Leave it out for development builds.

Packing loads the patches once to record their metadata, so it needs Java.

## `reseam bundle manifest`

```bash
reseam bundle manifest manifest.toml
```

Checks a `manifest.toml` and prints its bundle details as JSON. The Gradle build uses it to read the bundle name.

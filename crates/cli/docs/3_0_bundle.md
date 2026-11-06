---
title: Bundle
description: Start a bundle from the template, create signing keys, pack and sign bundles, list what is inside one, and fetch published releases.
---

# `reseam bundle`

You start a bundle with `init` and make its signing key with `keygen`. The Gradle build calls the rest for you. They are useful for checking a bundle by hand.

## `reseam bundle init`

```bash
reseam bundle init my-patches
```

Creates a bundle project in the folder `my-patches` from the newest [patch bundle template](https://git.reseam.app/reseam/patches-template), which it downloads. It names the bundle in `manifest.toml` and `settings.gradle.kts`, and sets the Reseam Gradle plugin to the version of the CLI you ran, so the bundle builds for the engine you test with.

The name is the bundle's name everywhere, including in patch references like `my-patches/<patch id>`. It uses lowercase letters and digits, separated by single hyphens. The folder must not exist yet.

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

## `reseam bundle pubkey`

```bash
reseam bundle pubkey --key ~/.reseam/bundle-signing.key
```

Prints the public key of a key file `keygen` made, for when you need it again.

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

Packs and signs a staging folder into a `.reseam` file. The Gradle build creates the staging folder and calls this for you. The folder holds `manifest.toml`, the patch `.jar` and extension `.dex` files, an optional `resources/` folder, and an optional `modules.json`.

`modules.json` lists the modules the payload is built from, which sets each patch's [fingerprint](/docs/authoring/bundles/#fingerprints): every payload file in exactly one module, with the modules each one uses. A module whose files are built from every module that uses them, like a shared shrunk library, gives the SHA-256 of what else they come from as `source`. Without the file, the whole payload is one module.

```json
[
  {"name": ":apps:example:patch", "files": ["example-patches.jar"], "uses": [":apps:example:extensions:feed"]},
  {"name": ":apps:example:extensions:feed", "files": ["example-feed.dex"], "uses": ["kotlin"]},
  {"name": "kotlin", "files": ["kotlin.dex"], "source": "0f9253e5…"}
]
```

`--version <version>` records the release in the signed manifest, where [`reseam release patches`](/docs/cli/release/) and the app's settings screen read it. Leave it out for development builds.

Without `--key`, the bundle is packed unsigned. Nothing loads an unsigned bundle until [`reseam bundle sign`](#reseam-bundle-sign) signs it.

Packing loads the patches once to record their metadata, so it needs Java, and it runs the bundle's code.

## `reseam bundle sign`

```bash
reseam bundle sign build/reseam/unsigned/my-patches.reseam --key ~/.reseam/bundle-signing.key --out my-patches.reseam
```

Signs a bundle that `pack` made without `--key`. Signing runs none of the bundle's code, so you can pack on one machine and sign on another that holds the key. It checks the manifest as loading a bundle does, and every file against its hash, and refuses a bundle that is already signed.

## `reseam bundle manifest`

```bash
reseam bundle manifest manifest.toml
```

Checks a `manifest.toml` and prints its bundle details as JSON. The Gradle build uses it to read the bundle name.

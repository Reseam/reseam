---
title: Publish
description: Write or update a patches.json release index.
---

# `reseam publish patches`

Writes a `patches.json` release index from a signed `.reseam` bundle. This is the file Reseam Manager and the Reseam API read to discover releases.

```bash
reseam publish patches \
  build/bundle/reseam-patches.reseam \
  --version v0.1.0 \
  --url https://reseam.app/releases/reseam-patches-v0.1.0.reseam
```

## What it does

1. Opens the bundle archive, reads `manifest.pubkey`, and verifies `manifest.sig` against it. Whether the key is trusted is not decided here; that check only happens when a client loads the bundle to apply it.
2. Reads bundle `name`, `author`, and `description` from `manifest.toml`. Records the public key as hex in the index.
3. If `--out` already exists, loads it and refuses to continue when the recorded `bundle.public_key` doesn't match the archive's key. This is the safety net against publishing a differently-keyed bundle to the same index.
4. Replaces any existing release with the same `--version`.
5. Inserts the new release at the top of `releases` and writes the file atomically (temp file, then rename).

## Arguments

| Argument | Purpose |
|----------|---------|
| `<bundle>` | Signed `.reseam` bundle to publish. |
| `--version <VERSION>` | Release version string. Non-empty. |
| `--url <URL>` | Public download URL for the bundle. |
| `--out <PATH>` | Index path. Defaults to `patches.json` in the current directory. |
| `--description <TEXT>` | Release description. |
| `--description-file <PATH>` | Description loaded from a file. Mutually exclusive with `--description`. |
| `--homepage <URL>` | Bundle homepage. When omitted, the existing homepage is preserved. |
| `--created-at <ISO8601>` | Release timestamp. Defaults to the current UTC time. Must be RFC 3339. |
| `--prerelease` | Mark the release as a prerelease. |

## Index shape

```json
{
  "bundle": {
    "name": "example-bundle",
    "author": "example",
    "description": "Example patches",
    "homepage": "https://reseam.app",
    "public_key": "1f3c..."
  },
  "releases": [
    {
      "version": "v0.1.0",
      "created_at": "2026-04-19T12:00:00Z",
      "description": "Initial release.",
      "download_url": "https://reseam.app/releases/example-bundle-v0.1.0.reseam",
      "prerelease": false
    }
  ]
}
```

The Reseam API reads this file from whatever URL you configure in `PATCHES_URL` and serves it back under `*.reseam.app` paths.

## `reseam publish manager`

Writes `manager.json`, the index Reseam Manager checks for updates and the website reads for downloads. Same shape as `patches.json` without a public key.

```bash
reseam publish manager \
  --name "Reseam Manager" --author Reseam \
  --version 1.0.0 \
  --url https://git.reseam.app/reseam/manager/releases/tag/v1.0.0
```

| Argument | Purpose |
|----------|---------|
| `--name <TEXT>` | Publisher name shown to users. |
| `--author <TEXT>` | Publisher author. |
| `--summary <TEXT>` | Publisher description. Optional. |
| `--out <PATH>` | Index path. Defaults to `manager.json`. |

`--version`, `--url`, `--description`, `--description-file`, `--homepage`, `--created-at`, and `--prerelease` behave as for `publish patches`.

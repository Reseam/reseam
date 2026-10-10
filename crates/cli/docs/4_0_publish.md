---
title: Publish
description: Add a release to a patches.json or manager.json index.
---

# `reseam publish`

Writes the index file that tells Reseam Manager where your releases are. It only edits the file; you upload it and the release yourself.

## `reseam publish patches`

```bash
reseam publish patches my-patches.reseam \
  --version 1.2.0 \
  --url https://example.com/my-patches-1.2.0.reseam
```

Adds the release to `patches.json`, or creates the file. The bundle's name, author, public key, and patch list are read from the signed bundle. A release with the same version is replaced, and the newest release goes first. Only the newest release and the newest prerelease keep their patch lists; older releases keep their version, notes, and link.

It refuses to change the public key recorded in an existing index, so a bundle signed with a different key can't take over your index by mistake.

| Flag | |
|---|---|
| `--version <version>` | the release version; required |
| `--url <url>` | where the bundle can be downloaded; required |
| `--out <path>` | the index file (default `patches.json`) |
| `--description <text>`, `--description-file <path>` | release notes |
| `--homepage <url>` | your project's page |
| `--created-at <time>` | release time in RFC 3339 (default now) |
| `--prerelease` | mark the release as a prerelease |

The Gradle build wraps this as `./gradlew stageRelease -PreleaseTag=v1.2.0`, which builds the bundle and writes the index into `build/reseam/release/`. See [Publishing](/docs/authoring/publishing/).

## `reseam publish manager`

```bash
reseam publish manager --name "Reseam Manager" --author Reseam \
  --version 1.0.0 --url https://example.com/manager/v1.0.0
```

Writes `manager.json`, the index of Reseam Manager releases. It takes the same release flags, plus `--name`, `--author`, and an optional `--summary`. The default output is `manager.json`.

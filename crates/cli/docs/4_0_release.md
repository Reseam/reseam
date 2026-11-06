---
title: Release
description: Publish a release to a GitHub or Forgejo release, an S3 bucket, or a folder.
---

# `reseam release`

Publishes a release and its index. It reads the index of your latest release, adds the new release, and publishes both, so your release history lives where you publish.

## `reseam release patches`

```bash
reseam release patches build/reseam/my-patches.reseam --tag v1.2.0
```

Publishes a bundle and `patches.json`. The bundle's name, author, public key, version, and patch list are read from the signed bundle, so the bundle must be packed with `--version`. A release with the same version is replaced, and the newest release goes first. Only the newest release and the newest prerelease keep their patch lists; older releases keep their version, notes, and link.

It refuses to change the public key recorded in the published index, so a bundle signed with a different key can't take over your index by mistake.

Before publishing, it downloads the newest release the index lists and prints which apps the new bundle updates for people who patched with it: per app package, how many patches have a changed [fingerprint](/docs/authoring/bundles/#fingerprints), are new, or are gone. Universal patches count as `any app`.

| Flag | |
|---|---|
| `--homepage <url>` | your project's page (default: the one in the published index, or the repository page) |

## `reseam release manager`

```bash
reseam release manager build/release --tag v1.0.0 --name "Reseam Manager" --author Reseam \
  --download-url https://reseam.app/download/
```

Publishes every file in the folder and `manager.json`, the index of Reseam Manager releases. It takes `--name`, `--author`, `--download-url`, and an optional `--summary`. The version is the tag without its `v`.

## `reseam release files`

```bash
reseam release files dist --tag v1.0.0
```

Publishes every file in the folder, without an index.

## Targets

`--to` picks where a release goes. The default, `forge`, creates the GitHub or Forgejo release of the tag with every file attached. Run it in a workflow: it reads the repository from the variables GitHub and Forgejo set, and the token from `RELEASE_TOKEN`, which you set to `${{ github.token }}`. The release stays a draft until every file is attached, and running it again for a tag replaces that tag's release. For bundles, the notes start with an Add to Reseam Manager link to the repository.

`s3://<bucket>/<prefix>` uploads to any S3-compatible store, configured by `AWS_ENDPOINT_URL`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`, and `AWS_SECRET_ACCESS_KEY`. A folder path writes to that folder, for a static host. Both put the release's files in `<prefix>/<tag>/` and the index also in `<prefix>/`, where it always holds the latest. That copy is written last and, on S3, marked `no-cache`; release files are cached for a day.

## Release flags

Every `reseam release` command takes these.

| Flag | |
|---|---|
| `--tag <tag>` | the release's tag; required |
| `--to <target>` | `forge`, `s3://<bucket>/<prefix>`, or a folder (default `forge`) |
| `--public-url <url>` | where an S3 or folder target is served from; required for `patches`, whose index links the bundle |
| `--forge-release` | for S3 and folder targets, also publish a GitHub or Forgejo release with the notes |
| `--notes <path>` | release notes (default: the subjects of the commits since the previous tag) |
| `--keep <n>` | for S3 and folder targets, keep only the newest `n` releases; folders that aren't version tags stay |

A release whose version has a prerelease part, such as `1.2.0-beta.1`, is published as a prerelease. The default notes come from `git`, so run it in a checkout that has your tags.

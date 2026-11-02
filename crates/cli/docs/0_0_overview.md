---
title: Overview
description: What the reseam command does, and who it is for.
---

# Overview

`reseam` is Reseam's command-line tool. Patch authors use it to build, sign, test, and publish bundles. Anyone can use it to patch APKs on a computer without Reseam Manager.

| Command | What it does |
|---|---|
| [`reseam patch`](2_0_patch.md) | Applies patches from bundles to an APK and signs the result. |
| [`reseam perf`](2_5_perf.md) | Patches several times and reports how long each step took and how much memory it used. |
| [`reseam bundle`](3_0_bundle.md) | Starts bundles from the template, creates signing keys, packs bundles, lists what is in them, and fetches published releases. |
| [`reseam release`](4_0_release.md) | Publishes a release to a GitHub or Forgejo release, an S3 bucket, or a folder. |
| [`reseam info`](5_0_info.md) | Prints an app's package, version, and DEX, class, and method counts. |

`reseam patch`, `perf`, and `info` work on local files and never touch the network. `reseam bundle init` downloads the bundle template, `reseam bundle fetch` downloads a release, and `reseam release` uploads one. Add `--help` to any command for its flags.

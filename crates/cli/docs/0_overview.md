---
title: Overview
description: What the Reseam CLI is and what it does.
---

# Overview

`reseam` is the command-line patch engine. Patch authors use it to build and sign bundles; advanced users run it on a laptop to patch APKs without Reseam Manager.

The binary is named `reseam` and ships from the `reseam-cli` crate.

## What it does

- Applies a signed patch bundle to an APK (single or split) and signs the output with APK Signature Scheme v2.
- Validates a bundle against an APK without writing anything (`--dry-run`).
- Prints APK metadata: package, version, DEX file count, splits, class and method totals.
- Generates Ed25519 bundle signing seeds.
- Packs and signs a bundle staging directory into a `.reseam` archive.
- Lists the patches inside a bundle with their compatibility and options.
- Writes or updates a `patches.json` release index from a signed bundle.

## Surfaces it talks to

- Reads APKs and split APKs.
- Reads `.reseam` bundles. Signature verification happens on load; unsigned or untrusted bundles are refused.
- Writes patched APKs and sibling `.pk8` / `.der` key material next to the output when no key was supplied.
- Writes `patches.json` indexes for distribution via the Reseam API.

The CLI does not fetch anything over the network. Bundles and APKs come from local paths; the `publish patches` command only records the URL you supply.

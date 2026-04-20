---
title: Glossary
description: Terms used by the CLI, bundles, and the patch engine.
---

# Glossary

**APK**: the Android app archive the CLI reads and writes. Can be a single file or a split set (base plus config splits).

**Bundle**: a signed `.reseam` archive holding one or more patches plus the metadata needed to apply them. The unit of distribution. The CLI refuses to load a bundle whose signature doesn't verify against a trusted key.

**Patch**: a single named change applied to an app: "block ads", "force AMOLED dark". Patches live inside bundles and declare their compatibility, dependencies, and options.

**Bundle manifest**: `manifest.toml` inside the bundle. Holds the bundle name, author, description, format version, and the hash of every payload file. Signed by the bundle author's Ed25519 key.

**Trusted key**: an Ed25519 public key a client accepts as a valid bundle signer. Bundles carry their signer key inside the archive; the client verifies the signature, then applies its own trust policy for that key.

**Signing key (APK)**: the PKCS#8 keypair used to sign the patched APK with Signature Scheme v2. Separate from the bundle signing key. Supplied via `--key` and `--cert`, or generated automatically next to the output.

**Execution plan**: the set of enabled patches and their option values for one patch run. Built from the bundle's defaults and the `--enable`, `--disable`, and `--option` flags.

**Split APK**: Android's way of distributing a single app as several APKs (base plus config splits for architecture, density, or language). The CLI treats the set as one unit: patches apply across DEX files from every split.

**`patches.json`**: the release index for a bundle. Lists the bundle's identity, public key, and every release. Read by the Reseam API and Reseam Manager. Written or updated by `reseam publish patches`.

**DEX**: Dalvik executable. The bytecode format inside an APK. Patches operate on DEX methods, classes, and strings.

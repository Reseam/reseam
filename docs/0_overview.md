# Overview

Reseam applies community-written patches to Android apps on-device. This documentation is for patch authors.

## The pieces

- **The engine** (`reseam-patcher`). Loads a bundle, resolves patch order, runs each patch against a mutable APK.
- **A bundle**. A signed archive containing compiled patch JARs, DEX extensions, and a `manifest.toml`. The unit of distribution.
- **The CLI** (`reseam`). Packs bundles, signs them, applies them to APKs, generates release indexes.
- **The Manager**. The Android app end-users install to apply bundles.

## Distribution model

Anyone can build and host their own bundle. Reseam publishes official bundles too, but publishing isn't gated. Users decide which bundle authors they trust.

Fork the patch bundle template to start your own. Rename it, replace the example patches, build and publish.

## What a patch does

A patch runs against a `PatchRuntime` that exposes the target APK's state. From Kotlin, a patch can:

- Read and mutate DEX (classes, methods, instructions, fingerprint-driven lookups).
- Edit the Android manifest.
- Read and write resources and binary AXML.
- Add or remove files in the APK.
- Inject DEX extensions: Java or Kotlin code compiled separately to add runtime behavior.
- Declare dependencies on other patches in the same bundle.
- Declare options the user configures before applying.
- Declare compatibility (which packages and versions). Incompatible patches are skipped.

Patch execution runs once at patch time. For behavior the user should be able to toggle after installing (on/off without repatching), the standard pattern is a settings extension: the patch gates a method on a setting key, and a shared settings extension lets the user flip the value at runtime.

## Next

- [Setup](1_setup.md)
- [Bundle layout](2_bundle_layout.md)
- [Writing patches](4_writing_patches.md)

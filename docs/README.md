# Reseam

Reseam applies community-written patches to Android apps on-device. A bundle is the unit patch authors build, sign, and publish; the engine (`reseam-patcher`) loads a bundle, resolves patch order, and runs each patch against a mutable APK. Reseam Manager on Android invokes the engine for end users; the `reseam` CLI invokes it during development.

These docs are for patch authors. Read in order:

1. [Setup](1_setup.md). Prerequisites and first build.
2. [Bundles](2_bundles.md). How a bundle project is laid out on disk and what ships in the signed archive.
3. Patches. Writing Kotlin patches.
   1. [Overview](3_patches.md). The `patch(...)` DSL, metadata, lifecycle, options, dependencies.
   2. [Runtime](3_1_runtime.md). What `PatchRuntime` exposes: bytecode, manifest, resources, files, log.
   3. [Fingerprints](3_2_fingerprints.md). Matching methods by stable properties to survive obfuscation renames.
   4. [Instructions](3_3_instructions.md). Building DEX bytecode sequences with labels and branches.
4. [Extensions](4_extensions.md). Java or Kotlin code compiled to DEX and injected at patch time.
5. [Publishing](5_publish.md). Build, apply locally, benchmark, generate `patches.json`, host.

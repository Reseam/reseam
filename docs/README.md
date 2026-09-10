# Reseam

Reseam applies community-written patches to Android apps on-device. A bundle is the unit patch authors build, sign, and publish; the engine loads a bundle, resolves patch order, and runs each patch against a mutable APK. Reseam Manager on Android invokes the engine for end users; the `reseam` CLI invokes it during development.

These docs are for patch authors. Read in order:

1. [Setup](1_setup.md). Prerequisites and first build.
2. [Bundles](2_bundles.md). Project layout and what ships in the signed archive.
3. [Patches](3_patches.md). The `patch(...)` DSL: metadata, compatibility, dependencies, internal patches, options, settings.
4. [Targets](4_targets.md). Finding methods, classes, fields, and instructions.
5. [Code](5_code.md). Adding code to a method: `before`, `after`, `replace`, values, branches, gates.
6. [Runtime](6_runtime.md). Manifest, resources, files, XML, bytecode, log.
7. [Bindings](7_bindings.md). Views over obfuscated objects.
8. [Extensions](8_extensions.md). Java code the bundle ships into the app.
9. [Dex layer](9_dex.md). Raw methods, classes, instructions, and the instruction builder.
10. [Publishing](10_publish.md). Build, apply locally, benchmark, release.

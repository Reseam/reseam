# Reseam

Reseam applies community-written patches to Android apps on-device. A bundle is the unit patch authors build, sign, and publish; the engine loads a bundle, resolves patch order, and runs each patch against a mutable APK. Reseam Manager on Android invokes the engine for end users; the `reseam` CLI invokes it during development.

These docs are for patch authors. Read in order:

1. [Setup](1_setup.md). Install what you need and build once.
2. [Bundles](2_bundles.md). What you publish, and how the project is laid out.
3. [Patches](3_patches.md). Writing a patch: name, apps it applies to, dependencies, options, settings.
4. [Finding code in the app](4_targets.md). Fingerprints for methods, classes, fields, and single instructions.
5. [Changing methods](5_code.md). Running your code when a method starts, before it returns, or instead of it.
6. [Manifest, resources, and files](6_runtime.md). Everything in the APK that is not bytecode.
7. [Reading obfuscated objects](7_bindings.md). Getting values out of classes whose names change every release.
8. [Shipping your own code](8_extensions.md). Java compiled into the app.
9. [Raw bytecode](9_dex.md). Instructions and registers, for when nothing above fits.
10. [Publishing](10_publish.md). Build, test locally, release.

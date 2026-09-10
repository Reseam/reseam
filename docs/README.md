# Reseam

Reseam applies community-written patches to Android apps on the phone. Patch authors write patches in Kotlin, build them into a signed bundle, and publish it; Reseam Manager downloads the bundle and applies the patches the user picks.

The words used throughout:

- **Patch**: one change to one app that a user can switch on.
- **Bundle**: the signed file you publish, with every patch you maintain.
- **Target**: how a patch describes the code it changes without naming it, so it survives obfuscation and updates. ReVanced calls this a fingerprint.
- **Extension**: Java you ship in the bundle for the app to run.
- **Settings**: switches inside the patched app.
- **Engine**: the program that applies a bundle. Reseam Manager embeds it; the `reseam` CLI wraps it.

Read in order the first time:

1. [Setup](1_setup.md)
2. [Bundles](2_bundles.md)
3. [Your first patch](3_first_patch.md)
4. [Patches](4_patches.md)
5. [Finding code in the app](5_targets.md)
6. [Changing methods](6_code.md)
7. [Manifest, resources, and files](7_runtime.md)
8. [Reading obfuscated objects](8_bindings.md)
9. [Shipping your own code](9_extensions.md)
10. [Raw bytecode](10_dex.md)
11. [Publishing](11_publish.md)

Look things up afterwards in the [API reference](12_reference.md). [Coming from ReVanced](13_revanced.md) maps the vocabulary.

Warnings mark mistakes that compile and then fail at patch time or on the next app update.

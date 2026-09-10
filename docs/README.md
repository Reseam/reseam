# Reseam

Reseam applies community-written patches to Android apps on-device. A bundle is the unit patch authors build, sign, and publish; the engine loads a bundle, resolves patch order, and runs each patch against a mutable APK. Reseam Manager on Android invokes the engine for end users; the `reseam` CLI invokes it during development.

These docs are for patch authors. The words used throughout:

- **App**: an Android app a user already has, as an APK. Reseam never modifies the installed copy; it produces a patched copy that installs alongside.
- **Patch**: one change to one app that a user can switch on. Written in Kotlin against the Reseam SDK.
- **Bundle**: the file a patch author publishes, holding every patch they maintain plus the code those patches inject. Signed, so users can trust it.
- **Target**: how a patch describes the method, class, or instruction it wants to change, without naming it, so the description survives the app's obfuscation and updates. ReVanced calls this a fingerprint.
- **Extension**: Java the bundle ships and the engine links into the app when a patch calls it. For anything too big to express as a few instructions.
- **Settings**: switches inside the patched app that let the user turn parts of a patch on and off without patching again.
- **Engine**: the Rust program that loads a bundle, runs the patches, and writes the patched APK. Reseam Manager embeds it on the phone; the `reseam` CLI wraps it on a computer.

Read in order the first time:

1. [Setup](1_setup.md). Install what you need and build once.
2. [Bundles](2_bundles.md). What you publish, and how the project is laid out.
3. [Your first patch](3_first_patch.md). One patch end to end: find a method, change it, build, test, debug.
4. [Patches](4_patches.md). Writing a patch: name, apps it applies to, dependencies, options, settings, and what runs when.
5. [Finding code in the app](5_targets.md). Fingerprints for methods, classes, fields, and single instructions, and how searches cost.
6. [Changing methods](6_code.md). Running your code when a method starts, before it returns, or instead of it.
7. [Manifest, resources, and files](7_runtime.md). Everything in the APK that is not bytecode.
8. [Reading obfuscated objects](8_bindings.md). Getting values out of classes whose names change every release.
9. [Shipping your own code](9_extensions.md). Java compiled into the app.
10. [Raw bytecode](10_dex.md). Instructions and registers, for when nothing above fits.
11. [Publishing](11_publish.md). Build, test locally, release.

Look things up afterwards:

- [API reference](12_reference.md). Every public symbol, one line each.
- [Coming from ReVanced](13_revanced.md). What each ReVanced concept is called here.

Callouts marked **Pitfall** describe mistakes that compile and then fail at patch time or on the next app update.

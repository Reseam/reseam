---
description: The words used across these docs.
---

# Glossary

**ABI**: the kind of processor native code is built for, such as `arm64-v8a`. An APK keeps each ABI's native libraries in its own `lib/<abi>/` folder or split, and a phone loads one of them.

**APK**: the file an Android app is installed from. Apps from stores often come as split APKs, or bundled into one **APKM** or **XAPK** file.

**App runtime**: code the engine adds to a patched app the first time a patch uses it. It holds the settings screen, the settings storage, and the branded components extensions build with. Bundles share one copy.

**Binding**: a path through obfuscated objects, such as `post.attributes.videos[0].getUrl()`, described by structure so code blocks can read it. See [Reading obfuscated objects](12_bindings.md).

**Bundle**: a signed `.reseam` file with patches and extensions. The unit you publish, and users trust.

**Capture**: a value a point saves with `captureAs`, for code blocks to read with `capture(name)`.

**Code block**: the `{ }` after `before`, `after`, or `replace`, where you describe code to add to the app. It runs while patching, and each call in it adds instructions.

**Component**: one APK of an app that ships as several. `components()` lists them.

**DEX**: the bytecode format inside an APK.

**Engine**: the part of Reseam that applies a bundle to an APK. It runs in Reseam Manager, the CLI, and the browser patcher.

**Extension**: Kotlin or JavaScript code in a bundle that patches add to the app.

**Gate**: a group of hooks that only take effect while a toggle setting is on.

**Hermes**: the JavaScript engine of React Native apps, which ship their code as Hermes bytecode.

**Hook**: code a patch adds at a target: `before`, `after`, `replace`, or a constant return.

**Internal patch**: a patch with no name. It only runs as a dependency of another patch.

**Mount build**: output that is mounted over the app installed from the store on a rooted phone, instead of installed as its own app. It leaves out patches that edit the manifest.

**Obfuscation**: renaming an app's classes and methods to short, meaningless names. The names change with every release, which is why patches use targets.

**Option**: a value the user sets before patching.

**`patches.json`**: the index of a bundle's releases, download links, and public key. Reseam Manager reads it to get updates.

**Point**: one instruction inside a method, found by matching.

**Preset**: the starting selection of patches. `recommended` is the patches for the app that are on by default.

**Reference**: a patch's full identifier, `<bundle>/<id>`.

**Reseam Manager**: the Reseam app for Android, Windows, and Linux.

**Setting**: a value the user can change inside the patched app: a toggle, text, a folder, or a choice.

**Settings host**: the internal patch that adds a bundle's settings screen to one app.

**Signer**: the key a bundle is signed with. Users trust signers, not bundles.

**Split APK**: one part of an app that ships as several APK files.

**String table**: the strings of a module's `src/main/strings`, with their translations. A patch module's table names its patches, and a settings module's table ships in the app. See [Names in every language](4_patches.md#names-in-every-language).

**Target**: a description of a method, class, field, or function that Reseam searches the app for.

**Universal patch**: a patch with no `compatibleWith`. It works on any app, and starts switched off unless its author calls `enabledByDefault(true)`.

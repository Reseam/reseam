---
description: The words used across these docs.
---

# Glossary

**APK**: the file an Android app is installed from. Apps from stores often come as split APKs, or bundled as an **APKM** or **XAPK** file.

**Bundle**: a signed `.reseam` file with patches and extensions. The unit you publish and users trust.

**Code block**: the `{ }` after `before`, `after`, or `replace`, where you describe code to add to the app.

**DEX**: the bytecode format inside an APK.

**Engine**: the part of Reseam that applies a bundle to an APK. It runs in Reseam Manager, the CLI, and the browser patcher.

**Extension**: Java code in a bundle that patches add to the app.

**Gate**: patched code that checks a setting while the app runs.

**Internal patch**: a patch with no name. It only runs as a dependency of another patch.

**Obfuscation**: renaming an app's classes and methods to short meaningless names. It changes with every release, which is why patches use targets.

**Option**: a value the user sets before patching.

**`patches.json`**: the index of a bundle's releases, download links, and public key. Users add it to Reseam Manager to get updates.

**Point**: one instruction inside a method.

**Reference**: a patch's full identifier, `<bundle>/<id>`.

**Reseam Manager**: the Reseam app for Android, Windows, and Linux.

**Setting**: a switch inside the patched app.

**Signer**: the key a bundle is signed with. Users trust signers, not bundles.

**Split APK**: one part of an app that ships as several APK files.

**Target**: a description of a method, class, or field that Reseam searches the app for.

**Universal patch**: a patch with no `compatibleWith`. It works on any app and is never selected by default.

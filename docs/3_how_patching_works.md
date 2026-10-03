---
description: What happens between picking patches and installing the patched app.
---

# How patching works

## The pieces

- **Patch**: one change to one app, such as hiding ads. You write it in Kotlin.
- **Bundle**: a signed `.reseam` file holding your patches and any Java code they add to apps. You publish it.
- **Engine**: the part of Reseam that applies a bundle to an APK. It runs inside Reseam Manager on the phone or desktop, in the `reseam` CLI, and in the browser patcher at reseam.app.

## A patch run

1. **Open the app.** The engine reads the APK, or an APKM, XAPK, or set of split APKs.
2. **Check the bundles.** Each bundle carries its signer's public key and a signature over its contents. The engine checks the signature, then checks that the user trusts that signer. Untrusted bundles never load.
3. **Choose patches.** By default, the patches made for this app that are on by default. Users add or remove patches from there. Patches that work on any app are always opt-in.
4. **Run them.** Patches run in dependency order. Each one finds code in the app and changes it. A patch that fails is reported, and patches that depend on it are skipped.
5. **Write and sign.** If no patch failed, the engine writes the patched APK and signs it.

## When your code runs

Your Kotlin runs twice, at two different times:

- **When the bundle loads.** The engine reads the name, description, compatibility, and options of every patch. Nothing has looked at the app yet, so targets and app files are not available.
- **When the patch runs.** The `execute { }` block runs against the opened app. This is where targets are resolved and code is changed.

Code blocks such as `before { }` run at patch time and *emit* instructions into the app. A Kotlin `if` inside one decides what to emit; it does not become a branch in the app. [Changing code](8_changing_code.md) explains how to branch inside the app.

## Signing the patched app

Android only installs an update to an app if it is signed with the same key as the installed copy. Reseam Manager and the browser patcher create a signing key the first time they patch and keep using it. The CLI writes its key next to the output. If the key is lost, apps patched with it must be uninstalled before the next patched version installs.

## Versions

Bundles record the engine version that built them. An engine loads bundles from its own release line: the same major version, or the same minor version while the major is 0. When the line changes, rebuild your bundle with the matching plugin and CLI.

Next: [Declaring patches](4_patches.md).

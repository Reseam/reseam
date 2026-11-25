---
description: What happens between picking patches and installing the patched app.
---

# How patching works

## The pieces

- **Patch**: one change to one app, such as hiding ads. You write it in Kotlin.
- **Bundle**: a signed `.reseam` file that holds your patches, and any Kotlin code they add to apps. This is what you publish.
- **Engine**: the part of Reseam that applies a bundle to an APK. It runs inside Reseam Manager on a phone or computer, in the `reseam` CLI, and in the browser patcher at reseam.app.

## A patch run

1. **Open the app.** The engine reads the APK, or an APKM, XAPK, or set of split APKs.
2. **Check the bundles.** Each bundle carries its signer's public key and a signature over its contents. The engine checks the signature, then checks that the user trusts that signer. A bundle from an untrusted signer never loads.
3. **Choose patches.** By default, the run starts from the patches made for this app that are on by default. Patches that work on any app are off by default, unless their author says otherwise. Users add or remove patches from there.
4. **Check options.** The engine checks every option value against its declaration before any patch runs.
5. **Run the patches.** Patches run in dependency order. Each one finds code in the app and changes it. If a patch fails, the engine undoes what it changed, and skips the patches that depend on it.
6. **Write and sign.** If no patch failed, the engine writes the patched APK and signs it. If one failed, it writes nothing, unless the user asked for the output without the failed patches.

## When your code runs

Your Kotlin runs at two different times:

- **When the bundle loads.** The engine reads the name, description, compatibility, and options of every patch. It hasn't looked at the app yet, so targets and the app's files aren't available.
- **When the patch runs.** The `execute { }` block runs against the opened app. This is where targets are searched for and code is changed.

So keep anything that touches the app inside `execute { }`. A top-level value that resolves a target, or reads an option, fails when the bundle loads.

## Code blocks add code

Code blocks, such as `before { }`, also run while patching, and each call in them *adds instructions* to the app:

```kotlin
showAd.before {
    if (options[skipAll]) returnVoid()
}
```

The Kotlin `if` here runs once, while patching. It decides whether to add a `return` to the app at all. It never becomes a check in the app. To branch inside the app, use `whenTrue` and the other branches in [Changing code](7_changing_code.md#branches-and-returns).

## Signing the patched app

Android only installs an update to an app when it is signed with the same key as the installed copy.

- Reseam Manager and the browser patcher create a signing key the first time they patch, and keep using it.
- The CLI writes its key next to the output, and reuses it when you pass it back with `--key` and `--cert`.

If the key is lost, the patched app must be uninstalled before a version signed with a new key installs. Users lose its data.

## Versions

Bundles record the engine version that built them. An engine only loads bundles from its own release line: the same minor version while the major version is 0, and the same major version after that. When the line changes, rebuild your bundle with the matching plugin and CLI. [When Reseam updates](17_publishing.md#when-reseam-updates) covers what that means for a published bundle.

Next: [Declaring patches](4_patches.md).

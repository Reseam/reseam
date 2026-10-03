---
title: Patch
description: Apply patches from bundles to an APK and sign the result.
---

# `reseam patch`

Applies patches from one or more bundles to an app and writes a signed APK.

```bash
reseam patch app.apk --bundle patches.reseam --trust <public key> --output patched.apk
```

`--trust` takes the bundle signer's public key, as printed by `reseam bundle keygen` or `reseam bundle list`. A bundle whose signer you didn't pass is refused.

## Choosing patches

The run starts from the patches made for this app that are on by default. Change it with:

| Flag | |
|---|---|
| `--preset all` | start from every patch made for this app |
| `--preset none` | start from nothing |
| `--enable <patch>` | add a patch |
| `--disable <patch>` | remove a patch |

Name a patch by its display name, its ID, or its full reference `<bundle>/<id>`; `reseam bundle list` shows all three. If a name matches several patches, the error lists their references so you can pick one.

Patches that work on any app are never in a preset; add them with `--enable`. A patch made for other versions of the app is skipped; `--ignore-versions` runs it anyway.

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> \
  --preset none --enable "Hide ads" --enable "Allow screenshots"
```

## Options

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> \
  --option "Clone app.packageName=com.example.clone"
```

`--option <patch>.<key>=<value>`, repeatable. The value is read as the option's type: `true` or `false`, a number, or text. A list is comma-separated.

## Split APKs, APKM, and XAPK

```bash
reseam patch base.apk --split config.arm64_v8a.apk --split config.xxhdpi.apk \
  --bundle patches.reseam --trust <key> --output-dir patched/

reseam patch app.apkm --bundle patches.reseam --trust <key>
```

Pass extra splits with `--split`, or give an `.apkm` or `.xapk` file directly. XAPKs with OBB expansion files are not supported.

## Output

| | Default |
|---|---|
| One APK | `<name>-patched.apk` next to the input |
| Several APKs | a `<name>-patched/` folder next to the input |

`--output <file>` sets the file for a single APK. `--output-dir <dir>` sets the folder and works for either. The output is only written if no selected patch failed. `--dry-run` checks everything without patching or writing.

## Signing

Without `--key` and `--cert`, the CLI signs with the key next to the output: `<name>.pk8` and `<name>.der` beside a single APK, or `reseam.pk8` and `reseam.der` inside the output folder. If they don't exist yet, it creates them.

Keep that key. Android only installs an update over an app signed with the same key, so patch future versions with it, using `--key` and `--cert`:

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> \
  --key patched.pk8 --cert patched.der --output patched-2.apk
```

## All flags

| Flag | |
|---|---|
| `<APK>` | the APK, APKM, or XAPK to patch |
| `--bundle <path>` | a bundle to load; repeatable, required |
| `--trust <key>` | a signer to accept; repeatable |
| `--split <path>` | an extra split APK; repeatable |
| `--preset <preset>` | `recommended` (default), `all`, or `none` |
| `--enable <patch>`, `--disable <patch>` | add or remove a patch; repeatable |
| `--option <patch>.<key>=<value>` | set an option; repeatable |
| `--ignore-versions` | run patches on app versions they weren't made for |
| `--output <file>` | output file, for a single APK |
| `--output-dir <dir>` | output folder |
| `--key <pk8>`, `--cert <der>` | signing key and certificate, together |
| `--dry-run` | check without patching or writing |

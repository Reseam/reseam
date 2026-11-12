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

Patches that work on any app start switched off, unless their author enabled them by default. Add them with `--enable`. A patch made for other versions of the app is skipped, and `--ignore-versions` runs it anyway.

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> \
  --preset none --enable "Hide ads" --enable "Allow screenshots"
```

## Options

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> \
  --option "Clone app.packageName=com.example.clone"
```

`--option <patch>.<key>=<value>`, repeatable. The value is read as the option's type: `true` or `false`, a number, or text. For a list option, give `--option` once per item; each one adds an item.

## Split APKs, APKM, XAPK, and APKS

```bash
reseam patch base.apk --split config.arm64_v8a.apk --split config.xxhdpi.apk \
  --bundle patches.reseam --trust <key> --output-dir patched/

reseam patch app.apkm --bundle patches.reseam --trust <key>
```

Pass extra splits with `--split`, or give an `.apkm`, `.xapk` or `.apks` file directly. An `.apks` file is a ZIP with the APKs at its root, as `reseam patch` writes one. XAPKs with OBB expansion files are not supported.

## Builds for one phone

Most apps carry native libraries for several kinds of processor, and a phone loads only one of them. `--abi` builds for one phone, so the output keeps only the libraries that phone loads:

```bash
reseam patch app.apkm --bundle patches.reseam --trust <key> \
  --abi arm64-v8a --abi armeabi-v7a --abi armeabi
```

Android loads the first ABI in the phone's list that the app has libraries for, so give the ABIs in the phone's order. `adb shell getprop ro.product.cpu.abilist` prints that list. The output keeps that ABI's `lib/` folder, and from split input only that ABI's split. The log names the ABI it kept.

The output installs only on phones that run that ABI. Leave out `--abi` when you build an APK to share.

Apps that declare `android:multiArch` load every ABI the phone runs, so they keep all of them. A split that doesn't declare its ABI in `android:splitTypes` stays in the output, with the other ABIs' libraries removed.

## Output

| | Default |
|---|---|
| One APK | `<name>-patched.apk` next to the input |
| Several APKs | a `<name>-patched/` folder next to the input |

`--output <file>` sets the file for a single APK. Name it `.apks` to get every APK in one file instead, which works for either. `--output-dir <dir>` sets the folder, and works for either.

`--dry-run` checks the bundles, the selection, and the options without running any patch or writing anything. Targets are only searched when patches run, so a dry run doesn't tell you whether they match.

## When a patch fails

If any selected patch fails, `reseam patch` writes nothing, and exits with an error that names the failed patches.

To get an APK anyway, add `--leave-out-failed`. The output then leaves out the failed patches, and every patch that depends on them, as if you had never selected them.

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> --leave-out-failed
```

## Test builds

`--trace` makes a test build that shows which of your patch's changes the app runs. Every place a patch changed the app's code logs a line each time the app runs it:

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> --trace --output traced.apk
adb install -r traced.apk
adb logcat -v tag -s reseam-trace
```

Each line names the patch, the method, and the instruction where the change starts, counted as in a build without `--trace`. Targets match exactly what they match in a normal build. The extra calls slow down code that runs often, so don't give traced builds to users.

## Mount builds

`--mount` builds output to bind-mount over the app installed on a rooted phone, which keeps its signature and data. Android keeps the installed app's manifest, so patches that edit the manifest are left out, and the patches that need them are skipped. When patching finds such a patch, the CLI patches again from the original APK without it.

## Signing

Without `--key` and `--cert` or `--keystore`, the CLI signs with the key next to the output: `<name>.pk8` and `<name>.der` beside a single APK or `.apks` file, or `reseam.pk8` and `reseam.der` inside the output folder. If they don't exist yet, it creates them.

Keep that key. Android only installs an update over an app signed with the same key, so patch future versions with it, using `--key` and `--cert`:

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> \
  --key patched.pk8 --cert patched.der --output patched-2.apk
```

To sign with the key Reseam Manager or the web patcher backed up, pass the backup with `--keystore`. It reads PKCS#12 keystores (`.p12`), the format every Reseam app backs keys up in, and signs with the first private key. Set the password in `RESEAM_KEYSTORE_PASSWORD`, which keeps it out of your shell history, or with `--keystore-password`:

```bash
RESEAM_KEYSTORE_PASSWORD=… reseam patch app.apk --bundle patches.reseam --trust <key> \
  --keystore reseam-key.p12
```

## All flags

| Flag | |
|---|---|
| `<APK>` | the APK, APKM, XAPK, or APKS to patch |
| `--bundle <path>` | a bundle to load; repeatable, required |
| `--trust <key>` | a signer to accept; repeatable |
| `--split <path>` | an extra split APK; repeatable |
| `--preset <preset>` | `recommended` (default), `all`, or `none` |
| `--enable <patch>`, `--disable <patch>` | add or remove a patch; repeatable |
| `--option <patch>.<key>=<value>` | set an option; repeatable |
| `--ignore-versions` | run patches on app versions they weren't made for |
| `--output <file>` | output file: an APK for a single APK, or an `.apks` file for any input |
| `--output-dir <dir>` | output folder |
| `--key <pk8>`, `--cert <der>` | signing key and certificate, together |
| `--keystore <file>` | sign with the first key in a PKCS#12 keystore |
| `--keystore-password <password>` | the keystore's password; also read from `RESEAM_KEYSTORE_PASSWORD` |
| `--abi <abi>` | build for a phone with this ABI, keeping only the native libraries it loads; repeatable, in the phone's order |
| `--mount` | build for mounting over the installed app; leaves out patches that edit the manifest |
| `--leave-out-failed` | write the output without failed patches and the patches that need them |
| `--trace` | test build: log each place a patch changed the app's code when the app runs it |
| `--dry-run` | check the bundles, selection, and options, without patching or writing |

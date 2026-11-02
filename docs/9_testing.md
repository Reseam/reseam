---
description: Build, patch, and install in a loop, read what the engine reports, and find out why a patch didn't work.
---

# Testing and debugging

A patch can fail in three places: while the bundle builds, while the engine patches the app, or inside the patched app. This page covers the loop that catches all three, and what to read in each case.

## The loop

```bash
./gradlew bundle
reseam patch app.apk \
  --bundle build/reseam/my-patches.reseam \
  --trust <public key> \
  --preset none --enable "Hide ads" \
  --output patched.apk
adb install -r patched.apk
```

- `--preset none --enable "Hide ads"` runs only the patch you're working on, plus anything it depends on. That keeps each run fast, and the log short.
- The CLI signs the output with a key it keeps next to it, `patched.pk8` and `patched.der`. Keep writing to the same output, and each new build installs over the last with `adb install -r`, keeping the app's data.
- The first time, uninstall the app if it came from a store. Android won't install an app signed with a different key over it.

Test on the app versions your patch declares. If you pinned versions with `compatibleWith`, a run on any other version skips the patch. Add `--ignore-versions` to run it anyway.

## Reading the log

The CLI logs each patch as it runs, and ends with a summary:

```text
INFO patch{patch=my-patches/app.example.patches.hideAds}: patch started
INFO patch{patch=my-patches/app.example.patches.hideAds}: patch applied
INFO patch run finished applied=1 dependencies=1 skipped=0 unmountable=0 failed=0
```

| Count | Patches that |
|---|---|
| `applied` | you selected, and that ran |
| `dependencies` | you didn't select, but ran because a selected patch needs them |
| `skipped` | didn't run: the app or version doesn't match, or a dependency was skipped or failed. The log says which. |
| `unmountable` | were left out of a mount build, because they edit the manifest |
| `failed` | ran into an error and were undone |

Messages your patch writes with `log.info` and `log.warn` show in the log too. `log.debug` messages, and the method every target picked, only show with debug logging on:

```bash
RUST_LOG=info,reseam=debug reseam patch app.apk ...
```

```text
DEBUG patch{patch=my-patches/app.example.patches.hideAds}: showAd: Lcom/example/a/b;->c()V
```

## When a patch fails

A failed patch logs an error with the reason, and the run writes nothing:

```text
ERROR patch failed patch=my-patches/app.example.patches.hideAds reason="… No method matched 'showAd'. Searched 3 candidate(s).
Reasons: strings("ad_shown"): 3 candidate method(s)
Near misses: Lcom/example/a/b;->c()Z [missed: return type mismatch]"
Error: 1 patch(es) failed: my-patches/app.example.patches.hideAds
```

The reason is followed by a Kotlin stack trace. The message before it is what matters. For a target, read it like this:

- **`Searched N candidate(s)`**: how many methods the query's selective constraints found before filtering.
- **`Reasons`**: what each constraint matched. `0 candidate method(s)` on a constraint means nothing in the app fits it, often a string that changed in this version.
- **`Near misses`**: the closest methods, each with the constraint it failed. Usually one of these is the method you want, and the reason says which constraint is wrong.

If several methods match, the error lists them all. Add a constraint that tells them apart; see [Exactly one match](5_finding_code.md#exactly-one-match).

To check the rest of your bundle while one patch is broken, add `--leave-out-failed`. The run then writes the APK without the failed patches and the patches that need them.

## When the patch applied but nothing changed

The patch ran, so its target matched something. It may not be the code you meant. Check these in order:

1. **It matched the wrong method.** Turn on debug logging, or log `target.explain().winner`, and compare the method it picked with the one you found in jadx. This happens most with loose targets, and with `first()`.
2. **The app doesn't run that method here.** Apps often have two paths for the same thing, such as an old and a new feed. Look in jadx for other callers.
3. **The hook is in a gate, and the setting is off.** Open your settings screen in the app and check the toggle.
4. **A server decides.** Some features are switched on by the app's server. Changing the code that reads the flag may not be enough.

A traced build tells 2 apart from 3 and 4. Add `--trace`, install the output, and watch the log while you use the screen your patch changes:

```bash
reseam patch app.apk ... --trace --output patched.apk
adb install -r patched.apk
adb logcat -v tag -s reseam-trace
```

```text
I/reseam-trace: my-patches/app.example.patches.hideAds  Lcom/example/a/b;->c()V  [4]
```

Every place your patch changed the app's code logs a line each time the app runs it: the patch, the method, and the instruction where the change starts, counted as in a build without `--trace`. No line means the app took another path (2). A line and no change means the cause is past your code (3 or 4). Changes to Hermes bytecode don't log.

Keep `--trace` for test builds. The calls slow down code that runs often, and the log names every place you patched.

## When the patched app crashes

Read the crash from the phone:

```bash
adb logcat -b crash
```

| Error | Usually means |
|---|---|
| `NoClassDefFoundError`, `NoSuchMethodError` | Patched code calls something that isn't there. Check that the extension is in the bundle, and that your stubs match the real app. |
| `VerifyError`, or `Rejecting class` in the log | Android refused the patched code, for example a value of the wrong type passed to a call. Note which method and patch, and [report it](https://git.reseam.app/reseam/reseam/issues). Code blocks should never produce this. |
| `NullPointerException` in your extension | The app passed `null` where your function expects a value. Declare parameters nullable when the app can pass `null`. |
| `ClassCastException` | A `cast(type)` in a code block, or in a binding, met an object of another type. |

For your extension's own logic, `android.util.Log` works as in any Android app, and its messages show in `adb logcat`.

## Before you publish

- Patch the app versions you declare, and use the app on each: open the screens your patches change.
- Run with the patches users will get by default, not only the one you worked on, so you catch patches that conflict.
- If your bundle has settings, flip each toggle and check that the app follows.

Next: [Shipping your own code](10_extensions.md).

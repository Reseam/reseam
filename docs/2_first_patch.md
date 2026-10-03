---
description: Write a patch that removes a prompt from an app, build it, and test it.
---

# Your first patch

This page writes one patch from scratch: remove a "rate this app" prompt from an imaginary app, `com.example.app`.

## 1. Find the code

Open the APK in a decompiler such as [jadx](https://github.com/skylot/jadx) and find the method that shows the prompt.

App code is usually obfuscated: classes and methods get short meaningless names that change with every release. So don't note the method's name. Note what stays the same: the text it uses, what it returns, what it calls. Say the method loads the string `"rate_prompt_shown"` and returns nothing.

## 2. Describe it

Add a file under `apps/example/patch/src/main/kotlin/`:

```kotlin
package app.example.patches

import app.reseam.patch.Type
import app.reseam.patch.method

val showRatePrompt = method("showRatePrompt") {
    strings("rate_prompt_shown")
    returns(Type.Void)
}
```

This is a *target*: a description of the method that Reseam searches the app for. `"showRatePrompt"` is only a label for error messages.

## 3. Write the patch

```kotlin
import app.reseam.patch.patch

val hideRatePrompt = patch("Hide rate prompt") {
    description("Never asks you to rate the app.")
    compatibleWith("com.example.app")

    execute {
        showRatePrompt.before {
            returnVoid()
        }
    }
}
```

- `patch("Hide rate prompt")` is the name users see.
- `compatibleWith` says which app it is for.
- `execute` is the change: at the start of the method, return right away, so the prompt never shows.

## 4. Build and test

```bash
./gradlew bundle
reseam patch app.apk \
  --bundle build/reseam/my-patches.reseam \
  --trust <public key> \
  --output patched.apk
adb install patched.apk
```

The log ends with one line per patch: applied, skipped, or failed.

## 5. When the target doesn't match

If no method fits the description, the patch fails and says why:

```text
No method matched 'showRatePrompt'. Searched 3 candidate(s).
Near misses: Lcom/example/a/b;->c()Z [missed: return type mismatch]; ...
```

Here the nearest method returns a boolean (`Z`), so `returns(Type.Void)` was wrong. If several methods fit, the patch fails and lists them; add a constraint that tells them apart. [Finding code](6_finding_code.md) covers this in detail.

Next: [How patching works](3_how_patching_works.md).

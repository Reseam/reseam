---
description: Write a patch that removes a prompt from an app, build it, and test it.
---

# Your first patch

This page writes one patch from scratch. It removes a "rate this app" prompt from an imaginary app, `com.example.app`.

## 1. Find the code

Open the APK in a decompiler such as [jadx](https://github.com/skylot/jadx), and find the method that shows the prompt.

Most apps are obfuscated: their classes and methods get short, meaningless names like `a.b.c`, and the names change with every release. So don't write down the method's name. Write down what is likely to stay the same: the text it uses, what it returns, and what it calls.

Say the method loads the string `"rate_prompt_shown"` and returns nothing. That's enough to find it again in the next version.

## 2. Describe the method

Add a file under `apps/example/patch/src/main/kotlin/app/example/patches/`:

```kotlin
package app.example.patches

import app.reseam.patch.Type
import app.reseam.patch.method

val showRatePrompt = method("showRatePrompt") {
    strings("rate_prompt_shown")
    returns(Type.Void)
}
```

This is a *target*: a description of the method, which Reseam searches the app for when a patch uses it. `"showRatePrompt"` is a label for error messages. The app's real name for the method doesn't matter.

## 3. Name the patch

Users see a patch's name and description in their own language, so they live in a string table rather than in the code. Add `apps/example/patch/src/main/strings/values/strings.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="hide_rate_prompt">Hide rate prompt</string>
    <string name="hide_rate_prompt_description">Never asks you to rate the app.</string>
</resources>
```

Then tell the Gradle plugin which object to generate from it, in `apps/example/patch/build.gradle.kts`:

```kotlin
reseamStrings { objectName = "app.example.patches.ExamplePatchStrings" }
```

The build turns each string into a property of `ExamplePatchStrings`, so `hide_rate_prompt` becomes `ExamplePatchStrings.hideRatePrompt`. Translators add `values-de/strings.xml` and the like next to `values/` later, without touching the code.

## 4. Write the patch

In the file from step 2:

```kotlin
import app.reseam.patch.before
import app.reseam.patch.patch

val hideRatePrompt = patch(ExamplePatchStrings.hideRatePrompt) {
    description(ExamplePatchStrings.hideRatePromptDescription)
    compatibleWith("com.example.app")

    execute {
        showRatePrompt.before {
            returnVoid()
        }
    }
}
```

- `patch(ExamplePatchStrings.hideRatePrompt)` is the name users see.
- `compatibleWith` says which app the patch is for. It is selected by default when someone patches that app.
- `execute` is the change. Here, it adds `return` at the start of the method, so the prompt never shows.

The patch has to be a public, top-level `val`. That is how Reseam finds it.

Hooks such as `before` are extension functions, so each needs its import. Your IDE adds them for you.

## 5. Build and test

```bash
./gradlew bundle
reseam patch app.apk \
  --bundle build/reseam/my-patches.reseam \
  --trust <public key> \
  --output patched.apk
adb install patched.apk
```

The log reports each patch as it runs, and ends with a summary:

```text
INFO patch{patch=my-patches/app.example.patches.hideRatePrompt}: patch applied
INFO patch run finished applied=1 dependencies=0 skipped=0 unmountable=0 failed=0
```

Then open the app and check that the prompt is gone. A patch that applied changed the code its target matched. Only the app shows you whether that was the right code.

## 6. When the target doesn't match

If no method fits the description, the patch fails, nothing is written, and the log says why:

```text
ERROR patch failed patch=my-patches/app.example.patches.hideRatePrompt reason="… No method matched 'showRatePrompt'. Searched 3 candidate(s).
Near misses: Lcom/example/a/b;->c()Z [missed: return type mismatch]"
```

The near miss is a method that loads the string but returns a boolean (`Z`), so `returns(Type.Void)` was wrong. If several methods fit instead, the patch fails and lists them. Add a constraint that tells them apart.

[Finding code](5_finding_code.md) explains targets in detail, and [Testing and debugging](9_testing.md) covers reading these reports.

Next: [How patching works](3_how_patching_works.md).

# Your first patch

This page writes one patch from nothing to a patched APK. It removes a "rate this app" prompt from an imaginary app, `com.example.app`; every step is the same for a real one.

## 1. Create the bundle

```
my-bundle/
  manifest.toml
  settings.gradle.kts
  gradlew
  gradle/
  apps/example/patch/src/main/kotlin/app/example/patches/RatePromptPatch.kt
```

Copy `gradlew` and `gradle/` from any Gradle project, or run `gradle wrapper`. `settings.gradle.kts` and `manifest.toml` are the two files from [Bundles](2_bundles.md). The `example` directory name is yours; it becomes the jar name `example-patches.jar`.

## 2. Find what to change

Open the APK in a decompiler (jadx works) and find the code that shows the prompt. Method and class names are obfuscated and change every release, so note the things that do not change: string constants the method loads, its return type, its parameters, what it calls. Say the prompt is shown by a method that loads the string `"rate_prompt_shown"` and returns nothing.

## 3. Write the patch

```kotlin
package app.example.patches

import app.reseam.patch.Type
import app.reseam.patch.before
import app.reseam.patch.method
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

val showRatePrompt = method("showRatePrompt") {
    strings("rate_prompt_shown")
    returns(Type.Void)
}
```

Read it top to bottom: the patch says what it is and which app it is for, `execute` says what changes, and the target below says what to look for. `"showRatePrompt"` is only a label for error messages; the app's method has some other name.

`before { returnVoid() }` makes the method return as soon as it is entered. [Changing methods](6_code.md) has the other shapes.

## 4. Build and inspect

```bash
./gradlew bundle
reseam bundle list build/reseam/my-bundle.reseam --trust <PUBLIC_KEY_HEX>
```

`bundle list` prints the patch with its package and description. Nothing has touched the app yet: building only compiles the Kotlin.

## 5. Apply to the APK

```bash
reseam patch app.apk \
  --bundle build/reseam/my-bundle.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
adb install patched.apk
```

The log ends with one line per patch (`patch applied`, `patch skipped`, `patch failed`) and a summary. Every target that resolved is logged as a `patch log` line with `level=debug` and the method it picked, which is the quickest way to see that the description matched what you meant.

## 6. When it does not match

A target that finds nothing fails the patch with a report:

```
No method matched 'showRatePrompt'. Searched 3 candidate(s).
Reasons: strings("rate_prompt_shown"): 3 candidate method(s); no candidate satisfied the full structural query
Near misses: Lcom/example/a/b;->c()Z [missed: return type mismatch]; ...
```

Read the near misses: here the method returns a boolean, so `returns(Type.Void)` is wrong. A target that finds too many fails too:

```
2 methods matched 'showRatePrompt'; add constraints, rank them, or take first(): ...
```

Add a constraint that separates them (`params()`, `inClass(...)`, `calls(...)`), or `rankBy` when one is preferable. Inside `execute`, `showRatePrompt.explain()` returns the same report for a target that did resolve.

> **Pitfall.** Do not reach for `first()` to silence an ambiguity you have not understood. It picks the alphabetically first descriptor, which is a different method after the next obfuscation pass.

## 7. Iterate

Edit, `./gradlew bundle`, `reseam patch`. Building against a local engine checkout (`RESEAM_WORKSPACE`) picks up SDK changes too. `reseam perf` times the run once the patch works.

Next: [Patches](4_patches.md) for everything a patch can declare.

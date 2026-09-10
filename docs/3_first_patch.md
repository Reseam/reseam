# Your first patch

One patch from nothing to a patched APK: remove a "rate this app" prompt from an imaginary `com.example.app`.

## 1. Create the bundle

```text
my-bundle/
  manifest.toml
  settings.gradle.kts
  gradlew
  gradle/
  apps/example/patch/src/main/kotlin/app/example/patches/RatePromptPatch.kt
```

`settings.gradle.kts` and `manifest.toml` are the two files from [Bundles](2_bundles.md). `gradlew` and `gradle/` come from `gradle wrapper`.

## 2. Find what to change

Open the APK in a decompiler and find the code that shows the prompt. Names are obfuscated and change every release, so note what does not: the strings the method loads, its return type, its parameters, what it calls. Say the prompt is shown by a method that loads `"rate_prompt_shown"` and returns nothing.

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

The patch says what it is and which app it is for, `execute` says what changes, the target below says what to look for. `"showRatePrompt"` is a label for error messages; the app's method has some other name.

## 4. Build, apply, install

```bash
./gradlew bundle
reseam patch app.apk \
  --bundle build/reseam/my-bundle.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
adb install patched.apk
```

The log ends with one line per patch (`applied`, `skipped`, `failed`). Each resolved target is logged with `level=debug` and the method it picked.

## 5. When it does not match

No match fails the patch with a report:

```text
No method matched 'showRatePrompt'. Searched 3 candidate(s).
Reasons: strings("rate_prompt_shown"): 3 candidate method(s); no candidate satisfied the full structural query
Near misses: Lcom/example/a/b;->c()Z [missed: return type mismatch]; ...
```

The near miss says the method returns a boolean, so `returns(Type.Void)` was wrong. Too many matches fail too:

```text
2 methods matched 'showRatePrompt'; add constraints, rank them, or take first(): ...
```

Add a constraint that separates them, or `rankBy` when one is preferable. [Finding code in the app](5_targets.md#debugging-a-target) has the details.

Next: [Patches](4_patches.md).

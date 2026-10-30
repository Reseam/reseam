---
description: Write Kotlin that runs inside the patched app, and call it from patches.
---

# Shipping your own code

Anything longer than a few lines is easier to write as normal Kotlin than to emit call by call. Put it in an *extension*: a Kotlin module in your bundle. Reseam compiles it to a DEX file, and adds it to the app the first time a patch uses one of its classes.

## Write it

```text
apps/example/extensions/ads/
  src/main/kotlin/app/example/ads/AdBlocker.kt
  src/stubs/java/com/example/app/FeedItem.java
```

- `src/main/kotlin` is your code. It compiles against Android's `android.jar`, and runs inside the app.
- `src/stubs/java` holds Java stand-ins for app classes you need to compile against, with only the members you use. Stubs are never shipped: the real classes come from the app.

Code that several apps share goes in `shared/<name>/` instead. The Kotlin that hooks it in can sit next to it, in `shared/<name>/patch/`, and an app's patch module depends on it with `compileOnly(project(":shared:<name>:patch"))`.

## Export what patches call

Mark every class, method, and field that a patch refers to with `@Export`:

```kotlin
@file:JvmName("AdBlocker")

package app.example.ads

import android.content.Context
import app.reseam.runtime.Export

@Export
fun init(context: Context) { … }

@Export
fun onFeedLoad(items: MutableList<Any?>?) { … }
```

- Top-level functions in a file with `@file:JvmName` become static methods of that class. So do `@JvmStatic` members of an `object`.
- Use `@JvmField` or `const val` for fields a patch reads or writes.
- Mark a class itself when a patch names the class, for example to make an app class extend it.

The build generates a binding for each class with exports, in the patch module of the same app, or of the same `shared/` folder: `app.example.ads.bindings.AdBlocker`. Call it from code blocks:

```kotlin
import app.example.ads.bindings.AdBlocker

appEntry { AdBlocker.init(application) }
loadFeed.before { AdBlocker.onFeedLoad(thisObject.field(feedItems)) }
```

A binding member is named after the export. Two overloads need different binding names: `@Export(name = "addWindow")`. A renamed or removed export breaks your patch module's build, so mistakes show up before you patch anything.

> [!WARNING]
> Parameters a patch passes in come from the app's code. Declare them nullable when the app can pass `null`. Reseam compiles extensions without Kotlin's null checks, so a `null` in a non-null parameter only fails where it is used.

## Let a patch fill in a function

Some code can only be written while patching, because it depends on the app's obfuscated names, such as reading a field found with a [binding](12_bindings.md). Declare the function `external`, without a body, and call it like any other function:

```kotlin
@Export
external fun videoUrl(media: Any?): String?
```

Then the patch writes its body:

```kotlin
Media.videoUrl.implement { returnValue(media.member("videoUrl", param(0))) }
```

The rest of your extension stays plain Kotlin, and only the app-specific part is emitted.

An `external` function that no applied patch implemented throws `UnsatisfiedLinkError` when the app calls it. Make the patch that calls it depend on the patch that implements it.

`external` also declares JNI functions from a native library your patch adds to the APK. No patch implements those, and Android links them when the app loads the library.

## The Kotlin standard library

Each bundle ships one copy of the parts of the Kotlin standard library its extensions use, moved into a package of its own. Apps that contain Kotlin keep their own copy, and bundles patched into one app never share one. Two things follow:

- If an extension implements a Kotlin type for the app, such as the app's `kotlin.jvm.functions.Function0`, declare that type in `src/stubs/java`. Stubbed classes keep their names. Every other `kotlin.*` class is the bundle's own.
- Don't pass Kotlin function types, or other `kotlin.*` types, between extensions of different bundles.

Coroutines and other `kotlinx` libraries aren't supported.

## Dependencies

A module only needs a `build.gradle.kts` to declare dependencies:

```kotlin
dependencies {
    compileOnly(project(":shared:player-stubs"))
    implementation("org.lsposed.hiddenapibypass:hiddenapibypass:6.1")
}
```

- `implementation`: shipped inside your extension. Android libraries (`.aar`) work too, but only their classes are used, and libraries with native code are refused.
- `compileOnly`: expected to be there already, in the app or in another module of your bundle. Use it for libraries the app already contains, including Android libraries with native code.

## What extensions can use

Every extension also compiles against:

- **The [app runtime](reference_runtime.md)**, in `app.reseam.runtime`: settings, translated strings, and branded components. The engine adds it to the app, so extensions never ship it. A bundle that defines a class in that package fails to load.
- **Your [settings modules](11_settings.md#read-settings-in-extensions)**: the settings and strings of the extension's own folder, and of the extensions it uses.

Outside the `apps/` and `shared/` layout, a patch module adds extensions with `reseamExtensions(project(":my-extension"))`, and an extension adds a settings module with `reseamSettings(project(":settings"))`.

## When something clashes

Two extensions that define the same class differently fail the bundle when it loads. `reseam bundle list <bundle> --verbose` shows which DEX files a bundle contains.

Next: [Settings inside the app](11_settings.md).

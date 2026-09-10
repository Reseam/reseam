# Shipping your own code

When a patch needs more than a few instructions, for example a settings screen, a download manager, or an activity, write it as Java and ship it in the bundle. Reseam compiles it against `android.jar` into a DEX file and links that file into the app the first time a patch refers to one of its classes. These modules are called extensions.

![Build time on top: an extension module with src/main/java/DeletedArchive.java and compile-only stubs under src/stubs/java goes through d8 against android.jar and lands in the bundle as telegram-anti-delete.dex, one file per module. Patch time below: patch code declares object DeletedArchive : ExtClass and calls DeletedArchive.init from appEntry.before; the engine merges the extension DEX into the app on that first reference, plus every extension it refers to. Patches never name a DEX file.](extension-flow.svg)

## Modules

An extension is a directory under `apps/<app>/extensions/<name>/` or `shared/<name>/`:

```
apps/telegram/extensions/anti-delete/
  src/main/java/app/reseam/telegram/antidelete/DeletedArchive.java
  src/stubs/java/org/telegram/messenger/MessagesStorage.java
```

- `src/main/java`: compiled and dexed.
- `src/stubs/java`: compile-time stand-ins for classes the app already has. On the compile classpath, never dexed.

No build script is needed. To compile against another extension, add one:

```kotlin
dependencies {
    compileOnly(project(":shared:settings-runtime"))
}
```

> **Pitfall.** Dependencies between extensions are `compileOnly`. Each module is dexed from its own classes only, and the engine links the dependency's DEX at patch time from the references in yours, so nothing is gained by bundling it twice. A class that ends up in two extension modules fails the bundle at load with `class ... is defined by both`.

> **Pitfall.** Stubs stand in for classes the app already has. A stub with a method the real class lacks compiles fine and throws `NoSuchMethodError` in the patched app. Copy signatures from the decompiled app, and re-check them when the app updates.

## Linking

Patches never name a DEX file. Declare the classes a patch calls:

```kotlin
object DeletedArchive : ExtClass("app.reseam.telegram.antidelete.DeletedArchive") {
    val init = static("init", Type.Context)
}
```

The first time a patch refers to a class an extension defines, in emitted code, in a lookup, or in a stub implementation, the engine merges that extension's DEX into the app along with every other extension it refers to. A reference nothing defines is logged once as a warning. Two extensions defining the same class is an error at load time.

> **Pitfall.** The warning `... is not defined by the app or any extension in the bundle` almost always means the `ExtClass` name has a typo or the extension module is not in the bundle (check `reseam bundle list`, which prints the DEX files). The patch still applies; the app crashes with `NoClassDefFoundError` when the call runs.

## The settings runtime

Toggle gates call `app.reseam.runtime.settings.ReseamSettings`, provided by the `settings-runtime` shared extension. A bundle that uses toggles ships it; the engine links it when the first gate is emitted.

Next: [Raw bytecode](10_dex.md).

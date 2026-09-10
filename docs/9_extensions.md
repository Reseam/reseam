# Shipping your own code

When a patch needs more than a few instructions, write Java and ship it in the bundle. Reseam compiles it against `android.jar` into a DEX file and links that file into the app the first time a patch refers to one of its classes. These modules are extensions.

![Build time: an extension module with src/main/java and compile-only stubs under src/stubs/java goes through d8 and lands in the bundle as one DEX file. Patch time: patch code declares an ExtClass and calls it; the engine merges the extension DEX into the app on that first reference, plus every extension it refers to.](extension-flow.svg)

```text
apps/example/extensions/ads/
  src/main/java/app/example/ext/AdBlocker.java
  src/stubs/java/com/example/app/FeedItem.java
```

`src/main/java` is compiled and dexed. `src/stubs/java` holds compile-time stand-ins for classes the app already has and is never dexed. A module needs a build script only to compile against another extension: `compileOnly(project(":shared:settings-runtime"))`.

A patch declares the extension classes it calls:

```kotlin
object AdBlocker : ExtClass("app.example.ext.AdBlocker") {
    val init = static("init", Type.Context)
    val onFeedLoad = static("onFeedLoad", Type.List)
}
```

The first reference to a class an extension defines merges that DEX into the app, plus every extension it refers to. Two extensions defining one class fail the bundle at load. A reference nothing defines is logged once: `... is not defined by the app or any extension in the bundle`.

> [!WARNING]
> That warning means a typo in the `ExtClass` name or a module missing from the bundle (`reseam bundle list` prints the DEX files); the app crashes with `NoClassDefFoundError` when the call runs. Stubs are promises too: a stub method the real class lacks throws `NoSuchMethodError` in the app.

Toggle gates call `app.reseam.runtime.settings.ReseamSettings` from the `settings-runtime` shared extension; the engine links it when the first gate is emitted.

Next: [Raw bytecode](10_dex.md).

# Extensions

Extensions are Java compiled against `android.jar` into one DEX file the bundle ships. Use them when the patched app needs new code at runtime: activities, settings screens, or logic too large to emit inline.

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

## Linking

Patches never name a DEX file. Declare the classes a patch calls:

```kotlin
object DeletedArchive : ExtClass("app.reseam.telegram.antidelete.DeletedArchive") {
    val init = static("init", Type.Context)
}
```

The first time a patch refers to a class an extension defines, in emitted code, in a lookup, or in a stub implementation, the engine merges that extension's DEX into the app along with every other extension it refers to. A reference nothing defines is logged once as a warning. Two extensions defining the same class is an error at load time.

## The settings runtime

Toggle gates call `app.reseam.runtime.settings.ReseamSettings`, provided by the `settings-runtime` shared extension. A bundle that uses toggles ships it; the engine links it when the first gate is emitted.

Next: [Dex layer](9_dex.md).

---
description: Write Java that runs inside the patched app, and call it from patches.
---

# Shipping your own code

Anything longer than a few lines is easier to write in Java than to emit instruction by instruction. Put it in an *extension*: a Java module in your bundle. Reseam compiles it into a DEX file and adds it to the app the first time a patch uses one of its classes.

## Write it

```text
apps/example/extensions/ads/
  src/main/java/app/example/ext/AdBlocker.java
  src/stubs/java/com/example/app/FeedItem.java
```

- `src/main/java` is your code. It compiles against Android's `android.jar`.
- `src/stubs/java` holds stand-ins for app classes you need to compile against. Stubs are never shipped; the real classes come from the app.

Code used by several apps goes in `shared/<name>/` instead of under an app. The Kotlin that hooks it in can sit next to it, in `shared/<name>/patch/`, and app patch modules depend on that with `compileOnly(project(":shared:<name>:patch"))`.

## Dependencies

A module only needs a `build.gradle.kts` to declare dependencies:

```kotlin
dependencies {
    compileOnly(project(":shared:settings-runtime"))
    implementation("org.lsposed.hiddenapibypass:hiddenapibypass:6.1")
}
```

- `implementation`: shipped inside your extension. Android libraries (`.aar`) work too, but only their classes are used, and libraries with native code are refused.
- `compileOnly`: expected to be there already, in the app or in another module of your bundle. Use it for libraries the app already contains, including Android libraries with native code.

## Call it

Declare the classes and methods your patches call:

```kotlin
object AdBlocker : ExtClass("app.example.ext.AdBlocker") {
    val init by static(Type.Context)
    val onFeedLoad by static(Type.List)
}
```

Each member is named after its property. Pass `name = "..."` when the Java name differs. Then use them in code blocks with `call(AdBlocker.init, application)`. See [Changing code](8_changing_code.md#calling-your-own-code).

> [!WARNING]
> A declaration must match the Java method exactly, parameter and return types included, or the patch fails. If a class name is wrong or the module is missing from the bundle, the patch log warns that the class `is not defined by the app or any extension in the bundle`, and the app crashes with `NoClassDefFoundError` when the call runs. A stub method the real class lacks crashes the same way, with `NoSuchMethodError`.

Two extensions defining the same class fail the bundle when it loads. `reseam bundle list <bundle> --verbose` shows which DEX files a bundle contains.

Next: [Raw bytecode](12_bytecode.md).

---
description: Describe methods, classes, and fields by what they do, so patches survive app updates.
---

# Finding code

App code is renamed with every release, so a patch never refers to a method by its name. It describes the method instead: the text it uses, what it returns, what it calls. That description is a *target*. Reseam searches the app for the one method that fits.

Declare targets as top-level values. They are searched for the first time a patch uses them, and the result is reused for the rest of that patch.

## Methods

```kotlin
val isAd = method("isAd") {
    strings("ad_impression")
    returns(Type.Boolean)
}
```

The constraints you will use most:

| Constraint | Matches methods that |
|---|---|
| `strings("a", "b")` | load each of these exact strings |
| `stringsStartingWith("reel/")` | load a string starting with this |
| `literals(42)` | use each of these number constants |
| `returns(Type.Boolean)` | return this type |
| `params(Type.String, Type.Int)` | take exactly these parameters |
| `hasParam(type)`, `paramCount(n)` | take this parameter, or this many |
| `inClass(classTarget)` | are declared in this class |
| `calls(target)`, `calls { name("create") }` | call this method |
| `calledBy(target)` | are called by this method |
| `flags(AccessFlags.STATIC)` | have these access flags |
| `opcodeSequence(...)` | contain these instructions back to back |
| `custom { instructionCount < 10 }` | pass your own check |

Types can be written as `Type` constants, dotted names (`java.lang.String`), or descriptors (`Ljava/lang/String;`). The [reference](reference.md#targets) lists every constraint.

## Exactly one match

A method target must match exactly one method. If nothing matches, or several do, the patch fails and lists what it found. To settle a tie:

- add a constraint that only the right method meets,
- `rankBy("label") { ... }` to score candidates and take the single best one,
- `first()` to take whichever sorts first, only when the matches are interchangeable.

Use `methods { }` instead of `method { }` when you want every match.

> [!WARNING]
> Give every target something specific: a string, a number, a class, or a call. A target like `method { returns(Type.Boolean); paramCount(1) }` fits half the app, and adding `first()` to it picks a different method after the next app update.

## Classes and fields

```kotlin
val mainActivity = klass("com.example.app.MainActivity")
val onResume = mainActivity.method("onResume") { params() }
val prefs = mainActivity.fieldOfType("android.content.SharedPreferences")

val adLoader = klass("adLoader") {
    strings("ad_unit_id", "ad_request_failed")
}
```

`klass("full.Name")` looks a class up by name; that works for classes the app doesn't obfuscate. `klass("label") { }` searches by description, like `method`. A class target narrows `method`, `methods`, `field`, and `fieldOfType` to that class. Add `inherited = true` to include methods the class inherits.

## Your own code is left out

Searches only cover the app's own code, not the Java your bundle adds to it (see [Shipping your own code](11_extensions.md)). Otherwise a match would depend on which patches ran before. Add `includeExtensions()` to search it too.

## When no query fits

`methodTarget`, `classTarget`, and `fieldTarget` take a block that finds the result by hand, with the whole app available:

```kotlin
val adState = classTarget("adState") {
    bytecode.findClass(adCounterField.owner) ?: error("ad state class missing")
}
```

These are slow on large apps; keep them narrow.

## Debugging a target

A failed target reports how many methods it looked at, why each was rejected, and the closest misses. A target that resolved is logged at debug level with the method it picked. Inside `execute`, `target.explain()` returns the same report.

Common mistakes:

- `returns("boolean")` is not a type. Use `Type.Boolean` or `"Z"`.
- `strings` matches whole strings only. Use `stringsStartingWith` for a prefix.
- If every candidate is rejected with `class mismatch`, the class target in `inClass` found the wrong class.

Next: [Pointing at instructions](7_points.md).

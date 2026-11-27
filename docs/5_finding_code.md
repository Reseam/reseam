---
description: Describe methods, classes, and fields by what they do, so patches survive app updates.
---

# Finding code

An app's code is renamed with every release, so a patch never refers to a method by its name. It describes the method instead: the text it uses, what it returns, what it calls. That description is a *target*, and Reseam searches the app for the one method that fits it.

Declare targets as top-level values. A target is searched for the first time a patch uses it, and the result is kept for the rest of that patch. Since nothing is searched when you declare it, one target can refer to another.

## Methods

```kotlin
val isAd = method("isAd") {
    strings("ad_impression")
    returns(Type.Boolean)
}
```

The label, `"isAd"`, is only used in error messages. The constraints you'll use most:

| Constraint | Matches methods that |
|---|---|
| `strings("a", "b")` | load each of these exact strings |
| `stringsStartingWith("reel/")` | load a string starting with this |
| `stringsContaining("count: ")` | load, for each of these, a string that contains it |
| `literals(42)` | use each of these number constants |
| `returns(Type.Boolean)` | return this type |
| `params(Type.String, Type.Int)` | take exactly these parameters, in order |
| `hasParam(type)`, `param(0, type)`, `paramCount(n)` | take this parameter, this parameter at this position, or this many |
| `inClass(classTarget)` | are declared in this class |
| `calls(target)`, `calls { name("create") }` | call this method, or one matching the block |
| `calledBy(target)` | are called by this method |
| `accesses { owner(prefs.descriptor) }` | read or write a matching field |
| `flags(AccessFlags.STATIC)` | have these access flags |
| `opcodeSequence(...)` | contain these instructions back to back |
| `custom { instructionCount < 10 }` | pass your own check |

Write types as `Type` constants, dotted names (`java.lang.String`), or descriptors (`Ljava/lang/String;`). [API: Targets and points](reference_targets.md#methodquery) lists every constraint.

### How the search runs

Reseam keeps indexes of the app's strings, numbers, calls, and field accesses. The most selective constraint in your query picks the first candidates from an index, and the other constraints filter them.

`custom` and `opcodeSequence` can only filter. Pair them with something selective, such as a string, or the search checks every method in the app.

## Exactly one match

`method { }` must match exactly one method. If nothing matches, or several do, the patch fails and lists what it found. To settle a tie:

- Add a constraint that only the right method meets. This is almost always the best fix.
- Rank the candidates with `rankBy("label") { ... }`. The block returns a score, and the single highest score wins. A tie still fails.
- Take the best ranked candidate with `first()`. Only do this when the matches are interchangeable.
- Take the candidate the app's class loader meets first with `firstInAppOrder()`. Like `first()`, it settles the choice without saying why, so a constraint is still better when one exists.

> [!WARNING]
> Give every target something specific: a string, a number, a class, or a call. A target like `method { returns(Type.Boolean); paramCount(1) }` fits hundreds of methods, and `first()` on it picks a different one after the next app update.

## Several methods

`methods { }` matches every method that fits, best ranked first:

```kotlin
val adLoaders = methods("adLoaders") {
    strings("ad_unit_id")
    returns(Type.Void)
}

execute {
    adLoaders.forEach { alwaysReturn() }
}
```

`single { }` picks one of them by a check that needs the resolved methods, and fails unless exactly one passes:

```kotlin
val feedAdLoader = adLoaders.single { parameterTypes.size == 2 }
```

## Classes

```kotlin
val mainActivity = klass("com.example.app.MainActivity")

val adLoader = klass("adLoader") {
    strings("ad_unit_id", "ad_request_failed")
}
```

`klass("full.Name")` finds a class by its name. That works for classes obfuscation doesn't rename, such as activities named in the manifest. `klass("label") { }` searches by description, like `method`. [`ClassQuery`](reference_targets.md#classquery) lists its constraints, including `extends`, `implements`, `hasInstanceField`, and `sourceFile`.

A class target narrows other lookups to that class:

```kotlin
val onResume = mainActivity.method("onResume") { params() }
val loaders = adLoader.methods { returns(Type.Void) }
```

`method(name)` on a class matches by name, which only works for names that survive obfuscation. Add `inherited = true` to include methods the class inherits from other app classes.

## Fields

```kotlin
val prefs = mainActivity.fieldOfType("android.content.SharedPreferences")
val adCount = adLoader.field("count")
val adState = loadAds.field { type(Type.Int) }
```

- `fieldOfType(type)` is the one instance field of that type on the class.
- `field(name)` finds a field by name, for names that survive obfuscation.
- `method.field { }` is the one field that method reads or writes matching the block.

For a field on a class your bundle adds, write the reference out: `field(owner, name, type)`.

### Obfuscated enums

Obfuscation renames an enum's fields, but not the names its constants report through `name()`. Find enums by those names:

```kotlin
val plan = klass("plan") { enumConstants("FREE", "PREMIUM") }
val premium = plan.enumConstant("PREMIUM")

planGetter.replace { returnValue(staticField(premium)) }
```

`enumConstants(...)` finds the enum with these constants. `enumConstant(name)` is the field holding the constant whose `name()` is `name`.

## Your own code is left out

Searches only cover the app's own code, not the code your bundle adds to it. Otherwise, what a target matches would depend on which patches ran before it. Add `includeExtensions()` to a query to search your code too.

## When no query fits

`methodTarget`, `classTarget`, and `fieldTarget` take a block that finds the result by hand, with the whole app available:

```kotlin
val adState = classTarget("adState") {
    bytecode.findClass(adCounterField.owner) ?: error("ad state class missing")
}
```

These skip the indexes, so keep them narrow on large apps.

## Debugging a target

A failed target reports how many candidates it looked at, what emptied the search, and the closest misses with the reason each was rejected. A target that resolved is logged at debug level, with the method it picked.

To see what a target picked while your patch runs, log its report:

```kotlin
execute {
    log.info(showAd.explain().winner)
}
```

Common mistakes:

- `returns("boolean")` is not a type. Use `Type.Boolean` or `"Z"`.
- `strings` only matches whole strings. Use `stringsStartingWith` for a prefix and `stringsContaining` for a part.
- If every candidate is rejected with `class mismatch`, the class target in `inClass` found the wrong class.

[Testing and debugging](9_testing.md) covers the rest of the loop.

Next: [Pointing at instructions](6_points.md).

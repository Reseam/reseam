# Finding code in the app

App code is obfuscated and renamed every release, so a patch never refers to a method by name. It describes the method: the strings it loads, its return type, what it calls. Reseam finds the one method matching the description. The description is a *target*; a target can be a method, a class, a field, or one instruction.

![Two releases of the same app on the left. The method is named xyz() in one and q() in the other, but both load the string ad_impression, return a boolean, and call bindFeedItem. On the right, a target declared as method("isAd") with strings("ad_impression") and returns(Type.Boolean) matches both releases. Exactly one method must match.](fingerprint-match.svg)

Targets are top-level values. They resolve the first time a patch uses them and stay cached for that patch. Types are accepted as descriptors (`Ljava/lang/String;`), dotted names (`java.lang.String`), or `Type` constants.

## Methods and classes

```kotlin
val isAd = method("isAd") {
    strings("ad_impression")
    returns(Type.Boolean)
}

val mainActivity = klass("com.example.app.MainActivity")
val onResume = mainActivity.method("onResume") { params() }
val prefs = mainActivity.fieldOfType("android.content.SharedPreferences")

val adLoaderClass = klass("adLoaderClass") {
    strings("ad_unit_id", "ad_request_failed")
}
```

`klass(name)` is a known class; `klass(label) { }` is a query. A class target scopes `method`, `methods`, `field`, and `fieldOfType`. The full constraint list is in the [reference](12_reference.md#targets).

Exactly one method must match. More than one is an error listing the candidates, unless `rankBy` produces a single best score or `first()` is set. `methods { }` returns every match instead.

## How a query searches

The engine indexes the app once per patch run. A query starts from the constraints that select fewest candidates and checks the rest against them:

1. `inClass`, `calls`, `calledBy`, `strings`, `literals` seed from the indexes, smallest first.
2. Failing those, `name`.
3. Failing those, `returns`, `params`, `hasParam`, `opcode`, which match thousands of methods in a large app.
4. With nothing selective, every method is a candidate.

> [!WARNING]
> Give every query a string, a literal, a class, or a call relationship. `method { returns(Type.Boolean); paramCount(1) }` considers most of the app and finds several matches. `first()` then picks whichever sorts first by descriptor, a different method after the next obfuscation pass; use it only for interchangeable matches. `methodTarget { }` and loops over `bytecode.classes` are unindexed hand scans: last resort, kept narrow.

## Points

A point is one instruction in a method:

```kotlin
val premiumCheck = mainActivity
    .method("onCreate") { strings("premium_status") }
    .point { string("premium_status") }
    .previous { invokeStatic { params(USER_SESSION); returns(Type.Boolean) } }
    .callee()
```

`point { }` matches the first instruction satisfying the block; `then { }` continues a sequence. `previous { }` and `next { }` walk from it, `callee()` and `field()` turn it back into a method or field target, `captureAs("name")` saves the register it writes for `capture("name")` in later code.

> [!WARNING]
> A point is an instruction index. Inserting code above it shifts the instruction. Emit at points before emitting at the method's entry, or resolve every point first (`captureAs`, `before`, or reading `index` resolves it).

## Custom targets

`appEntry` is `onCreate()` of the app's `Application` class, added if missing. `methodTarget`, `classTarget`, and `fieldTarget` take a block with the runtime as receiver for lookups no query expresses:

```kotlin
val adStateClass = classTarget("adStateClass") {
    bytecode.findClass(adCounterField.owner) ?: error("ad state class missing")
}
```

## Debugging a target

A failed target prints how many candidates were considered, which constraint seeded and rejected them, and the nearest misses with the reason each lost. A resolved target is logged with `level=debug` and its winner; `target.explain()` returns the same report inside `execute`.

- `returns("boolean")` is not a type; `Type.Boolean` or `"Z"` is.
- `strings` matches whole constants. Only `point { stringContains(...) }` matches parts.
- `class mismatch` on every candidate means the `inClass` target resolved to the wrong class.

Next: [Changing methods](6_code.md).

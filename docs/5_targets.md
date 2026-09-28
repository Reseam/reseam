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

1. `inClass`, `calls(target)` / `calls(ref)`, `calledBy`, `strings`, `stringsStartingWith`, `literals` seed from the indexes, smallest first.
2. Failing those, `calls { }` matches reference IDs natively and searches their indexed call sites.
3. Failing those, `name`.
4. Failing those, `returns`, `params`, `hasParam`, `opcode`, `opcodeSequence`, which match thousands of methods in a large app.
5. With nothing selective, every method is a candidate.

`custom { }` filters the candidates the other constraints selected; alone, it checks every method. A class query seeds from `strings` and `sourceFile`, then `hasInstanceField`.

`calls { }` reuses the `MethodRefMatch` API from instruction points:

```kotlin
val dialogFactory = method("dialog factory") {
    calls { owner("android.app.AlertDialog\$Builder"); name("create"); params() }
}
```

A call query needs an owner, name, return type, or exact parameter list. Platform methods need no declaring class in the APK. If another constraint narrowed the search, the matcher checks those candidates; otherwise the engine searches its call index and returns the matching methods. `callsMethod { }` reads references from candidate bodies and never seeds the query; use it for constraints `calls { }` cannot express.

`hasInstanceField` checks field declarations in one engine call, without reading method bodies. Static fields do not match.

A query searches the app's own code. Classes an [extension](9_extensions.md) ships are linked into the app on first reference, so leaving them in would make a match depend on which patch ran first; `includeExtensions()` in the query puts them back in, and `inClass` already names its owner. `klass(name)`, `ExtClass.target`, and `ExtMethod.target` are lookups by name, not queries, and always find the extension.

```kotlin
val playabilityGuard = method("playabilityGuard") {
    strings("Playability status")
    custom { instructionCount < 10 }
}

val treeObserver = method("treeObserver") {
    inClass(binderClass)
    flags(AccessFlags.CONSTRUCTOR)
    opcodeSequence(Opcode.CHECK_CAST, Opcode.NEW_INSTANCE, null, Opcode.INVOKE_VIRTUAL)
}
```

`opcode` asks only that the method contain each opcode somewhere. `opcodeSequence` asks for them back to back, `null` standing for any one instruction, which is what separates two methods built from the same parts. `custom { }` takes the candidate as its receiver, a `Method` or a `DexClass`, for the constraint the DSL has no word for.

`inClass` matches the methods a class declares. To include inherited methods, use `inClass(target, inherited = true)` or `klass.method("findPreference", inherited = true)`. A failed lookup without it reports how many inherited methods it skipped. `point { }.callee()` resolves the nearest declaration up the superclass chain.

> [!WARNING]
> Give every query a string, a literal, a class, or a call relationship. `method { returns(Type.Boolean); paramCount(1) }` considers most of the app and finds several matches. `first()` then picks whichever sorts first by descriptor, a different method after the next obfuscation pass; use it only for interchangeable matches. `methodTarget { }` and loops over `bytecode.classes` are unindexed hand scans: last resort, kept narrow, and they do see extension classes.

## Points

A point is one instruction in a method:

```kotlin
val premiumCheck = mainActivity
    .method("onCreate") { strings("premium_status") }
    .point { string("premium_status") }
    .previous { invokeStatic { params(USER_SESSION); returns(Type.Boolean) } }
    .callee()
```

`point { }` matches the first instruction satisfying the block; `then { }` continues a sequence. `previous { }` and `next { }` walk from it, `callee()` and `field()` turn it back into a method or field target. `captureAs("name")` saves the register it writes for `capture("name")` in later code. On an invoke, `captureArgumentAs("name", n)` saves the register of argument `n` instead: the receiver is argument 0 of an instance call, and a wide argument counts once. `writer(n)` is the instruction that wrote argument `n`, as a point of its own. Register-to-register moves are followed back to the instruction that produced the value. It fails when the value came in as a parameter or was written on more than one path into the call, naming the paths.

A point names its instruction, not an index. Code inserted anywhere in the method after the point resolved, an entry hook included, moves the point with its instruction, so points and method hooks can be emitted in any order. Replacing the body (`replace`, `alwaysReturn`, `setInstructions`) leaves nothing for the point to name, and its next use fails with `the method body was replaced under it`.

### Selecting multiple points

`points { }` uses the same matcher as `point { }` and selects every match in one method or a `MethodsTarget`. It reads each method once. `.all` resolves the points; `.forEach { }` resolves them all before editing; `.single()` defers resolution and requires exactly one match, listing candidates on failure.

```kotlin
val lookups = methods("attribution methods") {
    literals(attributionId)
}.points("attribution lookup") {
    invokeVirtual { name("findViewById"); params(Type.Int); returns(Type.View) }
    argument(1) { literal(attributionId) }
}
lookups.forEach {
    next { resultOf(Type.View) }.captureAs("view").after {
        call(Ads.hideAttribution, capture("view"))
    }
}
```

`argument(n) { }` matches the source of an invoke argument with the same literal, string, field, and result predicates. As with `writer` and `captureArgumentAs`, an instance receiver is argument 0 and a wide value counts once. Copies are followed across branches and exception handlers; every source must match. Equal constants on separate paths match, conflicting constants do not. Both words of a wide argument must have consistent origins.

Incoming parameters, unreachable values, cyclic copies, and unsupported control flow do not match; `explain().reasons` reports why. `writer` requires one instruction that wrote the argument and does not follow copies.

`next { resultOf(type) }` on an invoke requires its immediate result and fails if it is unused. `ownerAssignableTo("android.app.Dialog")` matches known subclasses too. It follows app classes, interfaces, and their external superclass names; unknown Android relationships do not match. Known `java.*` relationships use the host platform classes.

## Custom targets

`methodTarget`, `classTarget`, and `fieldTarget` take a block with the runtime as receiver for lookups no query expresses:

```kotlin
val adStateClass = classTarget("adStateClass") {
    bytecode.findClass(adCounterField.owner) ?: error("ad state class missing")
}
```

## Debugging a target

A failed target prints how many candidates were considered, which constraint seeded and rejected them, and the nearest misses with the reason each lost. A resolved target is logged with `level=debug` and its winner; `target.explain()` returns the same report inside `execute`.

- `returns("boolean")` is not a type; `Type.Boolean` or `"Z"` is.
- `strings` matches whole constants. `stringsStartingWith("reel/")` finds methods loading literals with that prefix through the string index; it requires a nonempty prefix. `point { stringContains(...) }` matches substrings at individual instructions.
- `class mismatch` on every candidate means the `inClass` target resolved to the wrong class.

Next: [Changing methods](6_code.md).

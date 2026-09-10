# Finding code in the app

App code is obfuscated and method names change with every release, so a patch never refers to a method by name. It describes the method instead: the strings it loads, its return type, what it calls. Reseam searches the app for the one method matching that description. ReVanced calls the description a fingerprint; here it is a *target*, and a target can be a method, a class, a field, or one instruction inside a method.

![Two releases of the same app on the left, 19.42 and 20.08. The method is named xyz() in one and q() in the other, but both load the string ad_impression, return a boolean, and call bindFeedItem. On the right, a target declared as method("isAd") with strings("ad_impression") and returns(Type.Boolean) matches both releases. Names change between releases; strings, types, and calls usually stay. Exactly one method must match.](fingerprint-match.svg)

Targets are top-level values. They resolve the first time a patch uses them and stay cached for the rest of that patch. Outside a running patch a target is only a description: reading `target.method` at load time throws.

Every type is accepted as a descriptor (`Ljava/lang/String;`), a dotted name (`java.lang.String`), or a `Type` constant (`Type.String`, `Type.Boolean`, `Type.Void`, `Type.List`, `Type.Context`, and so on). Arrays take a `[]` suffix or a leading `[`.

## Methods

```kotlin
import app.reseam.patch.method

val isAd = method("isAd") {
    strings("ad_impression")
    returns(Type.Boolean)
}
```

The string is a label for reports and errors. The block runs when the target resolves, so it may read other targets.

| Constraint | Meaning |
|---|---|
| `name("onReceive")` | Exact method name. |
| `strings("a", "b")` | Loads every listed string constant. |
| `literals(42L)` | Loads every listed numeric literal. |
| `returns(type)` | Return type. |
| `params(a, b)` | Exact parameter list; `params()` means none. |
| `param(2, type)` | The parameter at an index. |
| `hasParam(type)` | A parameter of this type anywhere. |
| `paramCount(3)` | Number of parameters. |
| `flags(AccessFlags.STATIC)` | Access flags the method carries. |
| `inClass(classTarget)` | Declared in that class. |
| `calls(methodTarget)` | Invokes that method. |
| `calledBy(methodTarget)` | Invoked by that method. |
| `callsMethod { name == "keySet" }` | Invokes something matching the predicate on the `MethodRef`, in the app or the platform. |
| `opcode(Opcode.INVOKE_STATIC)` | Contains the opcode. |
| `rankBy("label") { ... }` | Scores candidates; the highest wins. |
| `first()` | Take the best candidate even when several tie. |

Exactly one method must satisfy the query. More than one is an error listing the candidates, unless `rankBy` produces a single best score or `first()` is set.

### How a query searches

The engine indexes the app once per patch run: strings, literals, names, return and parameter types, opcodes, and who calls whom. A query does not scan every method. It starts from the constraints that select fewest candidates and checks the rest against them:

1. `inClass`, `calls`, `calledBy`, `strings`, `literals` seed the candidate set from the indexes and intersect, smallest first.
2. With none of those, `name` seeds it.
3. With none of those either, `returns`, `params`, `hasParam`, and `opcode` seed it. These match thousands of methods in a large app.
4. With nothing selective at all, every method in the app is a candidate.

The report a failed target prints shows this pipeline: `strings("x"): 3 candidate method(s)`, then which constraint rejected the rest.

> [!WARNING]
> A query with only shape constraints, such as `method { returns(Type.Boolean); paramCount(1) }`, considers most of the app and almost always finds several matches. Give every query at least one of: a string the method loads, a literal, the class it is in, or a method it calls or is called by. Names are fine when scoped to a class.

> [!WARNING]
> `first()` accepts whichever candidate sorts first by descriptor. Use it only when the matches are interchangeable, for example identical overloads that all need the same change. For anything else, add a constraint or `rankBy` so the choice is explained in the report and survives an app update.

> [!WARNING]
> `methodTarget { }`, `classTarget { }`, and loops over `bytecode.classes` walk the app by hand and are not indexed. Reach for them when no query expresses the lookup, and keep them narrow: start from a class you already have rather than from `bytecode.classes`.

`rankBy` sees the candidate: `method`, `paramCount`, `type` (its class descriptor), `methods(proto)`, `zeroArgListGetters()`, `callSitesFollowedByCast(type, lookAhead)`.

`methods("label") { }` returns every match, best ranked first. Use `all`, `forEach { }`, or `single { predicate }` on it.

## Classes

```kotlin
import app.reseam.patch.klass

val mainActivity = klass("com.example.app.MainActivity")

val adLoaderClass = klass("adLoaderClass") {
    strings("ad_unit_id", "ad_request_failed")
}
```

Without a block, the argument is a class name. With one, it is a label and the block is a query: `strings`, `hasInstanceField(type)`, `extends(type)`, `implements(type)`, `rankBy`, `first`.

A class target scopes lookups:

```kotlin
val onResume = mainActivity.method("onResume") { params() }
val toolbar = mainActivity.field("toolbar")
val prefs = mainActivity.fieldOfType("android.content.SharedPreferences")
```

- `method(name) { }`: adds `inClass` and `name`; the block narrows overloads.
- `methods(label) { }`: every method of the class matching the block.
- `field(name)`: a field by name.
- `fieldOfType(type)`: the one instance field of that type.

`field(owner, name, type)` names a field without looking it up, for classes the bundle adds itself.

## Points

A point is one instruction in a method, found by matching:

```kotlin
import app.reseam.patch.point

val premiumCheck = mainActivity
    .method("onCreate") { strings("premium_status") }
    .point { string("premium_status") }
    .previous { invokeStatic { params(USER_SESSION); returns(Type.Boolean) } }
    .callee()
```

`point { }` finds the first instruction matching the block. Inside it:

- `opcode(...)`, `string(value)`, `stringContains(part)`, `literal(value)`, `type(descriptor)`, `checkCast(type)`, `newInstance(type)`.
- `invoke { }`, `invokeStatic { }`, `invokeVirtual { }`, `invokeInterface { }`, `invokeDirect { }` with `owner`, `name`, `returns`, `params`, `hasParam`, `paramCount` on the callee.
- `calls(methodTarget)`: an invoke of exactly that method.
- `field { owner; name; type }`: a field access.
- `resultOf(returns)`: a `move-result` whose invoke returns the type.
- `where { }`: a predicate over the raw instruction.
- `then(within = 1) { }`: the next step of a sequence; the point is the last step.

From a point: `previous { }` walks back to the nearest match, `next { }` walks forward. `callee()` is the invoked method as a target. `field()` is the accessed field as a target. `instruction` is the raw instruction, `index` its position.

`captureAs("name")` records the register the instruction writes, typed from the instruction, for `capture("name")` in code emitted at a later point. Pass a type when it cannot be inferred.

> [!WARNING]
> A point is an instruction index. Inserting code above it in the same method shifts the instruction, and the cached index now names something else. Emit at points before emitting at the method's entry, or resolve every point you need first: `captureAs` and `before` on a point resolve it; reading `point.index` does too. The [Points](6_code.md#points) example captures two points, then emits once.

> [!WARNING]
> `previous { }` takes one step; `next { }` takes a sequence and lands on its last step. Both fail when nothing matches; neither wraps around.

## Built-in and custom targets

`appEntry` is `onCreate()` of the `Application` subclass the manifest names. When the class does not override it, one calling `super.onCreate()` is added.

When no query fits, resolve by hand with the [raw bytecode layer](10_dex.md):

```kotlin
val adStateClass = classTarget("adStateClass") {
    bytecode.findClass(adCounterField.owner) ?: error("ad state class missing")
}
```

`methodTarget`, `classTarget`, and `fieldTarget` take a block with the runtime as receiver that returns a `Method`, `DexClass`, or `FieldRef`.

## Debugging a target

A target that does not resolve fails the patch with its report: how many candidates were considered, which constraints seeded and rejected them, and the nearest misses with the reason each lost. A target that resolves is logged as a `patch log` line with `level=debug` naming the winner, so the CLI output shows which method each target picked. Inside `execute`, `target.explain()` returns the same report for a resolved target.

When a method that clearly exists is not found:

- Check the type spelling. `returns("boolean")` is not a type; `Type.Boolean` or `"Z"` is. Dotted class names are accepted, primitives are single letters.
- Check the string exactly. `strings` matches whole constants, not substrings; `point { stringContains(...) }` matches parts, method queries do not.
- Check the class. `inClass` on a class target that itself resolves to the wrong class shows up as `class mismatch` on every candidate.

When two methods match, the report lists both descriptors. Open them in the decompiler and pick the constraint that tells them apart.

Next: [Changing methods](6_code.md).

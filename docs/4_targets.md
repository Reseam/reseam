# Finding code in the app

App code is obfuscated and method names change with every release, so a patch never refers to a method by name. It describes the method instead: the strings it loads, its return type, what it calls. Reseam searches the app for the one method matching that description. ReVanced calls the description a fingerprint; here it is a *target*, and a target can be a method, a class, a field, or one instruction inside a method.

![Two releases of the same app on the left, 19.42 and 20.08. The method is named xyz() in one and q() in the other, but both load the string sponsored_label, return a boolean, and call renderFeedItem. On the right, a target declared as method("isSponsored") with strings("sponsored_label") and returns(Type.Boolean) matches both releases. Names change between releases; strings, types, and calls usually stay. Exactly one method must match.](fingerprint-match.svg)

Targets are top-level values. They resolve the first time a patch uses them and stay cached for the rest of that patch.

Every type is accepted as a descriptor (`Ljava/lang/String;`), a dotted name (`java.lang.String`), or a `Type` constant (`Type.String`, `Type.Boolean`, `Type.Void`, `Type.List`, `Type.Context`, and so on). Arrays take a `[]` suffix or a leading `[`.

## Methods

```kotlin
import app.reseam.patch.method

val isSponsored = method("isSponsored") {
    strings("sponsored_label")
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

`rankBy` sees the candidate: `method`, `paramCount`, `type` (its class descriptor), `methods(proto)`, `zeroArgListGetters()`, `callSitesFollowedByCast(type, lookAhead)`.

`methods("label") { }` returns every match, best ranked first. Use `all`, `forEach { }`, or `single { predicate }` on it.

## Classes

```kotlin
import app.reseam.patch.klass

val secretMediaViewer = klass("org.telegram.ui.SecretMediaViewer")

val signatureCheckClass = klass("signatureCheckClass") {
    strings("The provider for uri '", "' is not trusted: ")
}
```

Without a block, the argument is a class name. With one, it is a label and the block is a query: `strings`, `hasInstanceField(type)`, `extends(type)`, `implements(type)`, `rankBy`, `first`.

A class target scopes lookups:

```kotlin
val openMedia = secretMediaViewer.method("openMedia") { param(0, "org.telegram.messenger.MessageObject") }
val windowLayoutParams = secretMediaViewer.field("windowLayoutParams")
val statusField = secretMediaViewer.fieldOfType("android.view.WindowManager\$LayoutParams")
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

val developerMenuGate = clearNotificationReceiver
    .method("onReceive") { strings("NOTIFICATION_DISMISSED") }
    .point { string("NOTIFICATION_DISMISSED") }
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

Resolve points before mutating their method, or declare them per use: an index goes stale once code is inserted above it.

## Built-in and custom targets

`appEntry` is `onCreate()` of the `Application` subclass the manifest names. When the class does not override it, one calling `super.onCreate()` is added.

When no query fits, resolve by hand with the [raw bytecode layer](9_dex.md):

```kotlin
val carouselStateClass = classTarget("carouselStateClass") {
    bytecode.findClass(carouselIndexField.owner) ?: error("carousel state class missing")
}
```

`methodTarget`, `classTarget`, and `fieldTarget` take a block with the runtime as receiver that returns a `Method`, `DexClass`, or `FieldRef`.

## Reports

`target.explain()` returns why the target resolved the way it did: the winner, how many candidates were considered, the reasons, and the near misses. A failed resolution throws with the same report. Every resolved target is logged at debug level with its winner.

Next: [Changing methods](5_code.md).

# Changing methods

Once a target is found, a patch adds code to it: when the method starts, before every return, or instead of the body. The code is Kotlin calls describing values; Reseam emits the Dalvik instructions and picks the registers.

```kotlin
execute {
    loadFeed.before {
        call(AdBlocker.onFeedLoad, thisObject.field(feedItems))
    }

    buildTitle.after {
        returnValue(call(Badges.append, capture("result"), param(1)))
    }

    configureDownloads.replace {
        thisObject.set(maxParallelDownloads, int(8))
        returnVoid()
    }
}
```

`before`, `after`, `replace` on a method mean entry, every return, whole body. On a point they mean just before or after that instruction. In `after`, `capture("result")` is the value being returned. Shortcuts: `alwaysReturn(...)`, `alwaysReturnNull()`, `replaceAllStrings`, `replaceAllLiterals`.

In a method's `after` block, `param(i)`, `paramOfType(type)`, `lastParam`, and `thisObject` read values saved at method entry. Only the values referenced by the block are saved, in dedicated locals shared by its return sites. This remains safe when the original method overwrites or reuses its incoming registers. Object values are saved references, not copies of the objects. Assigning to a saved parameter changes that local; use `capture("result").assign(...)` or `returnValue(...)` to change what the method returns.

Point hooks read registers at the selected instruction. They do not snapshot entry arguments; use a point capture when you need a value produced by the method body.

> [!WARNING]
> The Kotlin in a code block runs once at patch time; each call adds instructions. A Kotlin `if` chooses what to emit, it does not branch in the app. Use `whenTrue` and friends for branches. Values belong to the block that made them; passing a `param(0)` into another block fails with `ValueRef belongs to a different code block`.

## Values and branches

`thisObject`, `param(i)`, constants (`int`, `bool`, `string`, `nullObject`), `staticField`, `enumValue`, `newInstance`, and `call(...)` produce values. `setStatic(field, value)` writes a static field. On a value: `cast`, `field`, `set`, `assign` (overwrite in place), `call` for instance methods, `size`, `get`, `plus`, `minus`. The [reference](12_reference.md#changing-methods) lists them all.

A value read with `field` or `staticField` is a copy; write the field with `set` or `setStatic`, since `assign` on the copy fails the patch. Access is checked too: a private member of another class, or a package-private member or non-public class in another package, fails at patch time.

```kotlin
whenEnabled(AppSettings.fasterDownloads) {
    thisObject.set(maxParallelDownloads, int(8))
} otherwise {
    thisObject.set(maxParallelDownloads, int(4))
}
```

`whenTrue`, `whenFalse`, `whenNull`, `whenNotNull`, `whenEqual`, `whenNotEqual`, `whenInstanceOf(value, type)`, each with an optional `otherwise { }`. Null and false are both a zero test; `whenEqual` is reference equality; null is not an instance of any type. `returnVoid`, `returnValue`, `returnTrue`, `returnFalse`, `returnNull` pick the return instruction from the type.

## Gates

A toggle from [Settings](4_patches.md#settings) gates code at runtime:

```kotlin
sendTypingEvent.returnFalseWhen(AppSettings.hideTyping)

isPremiumUser.before(AppSettings.unlockFeatures) {
    whenTrue(call(isCurrentUser, param(0))) { returnTrue() }
}
```

`before(toggle)` and `after(toggle)` wrap the block in `whenEnabled`. `skipWhen` (void), `returnTrueWhen`, `returnFalseWhen` (boolean), `returnNullWhen` (object) are the common shapes. All in `app.reseam.patch.settings`.

## Calling your own code

`call(AdBlocker.init, application)` calls a method of an [extension](9_extensions.md); the first reference links its DEX into the app. Static methods are `call(ext, args)`, instance methods `receiver.call(ext, args)`. `ext.implement { }` replaces the extension method's body with emitted code, usually from a [binding](8_bindings.md). A missing extension method or an argument count that disagrees with its proto fails the patch.

> [!WARNING]
> An `ExtClass` declaration must match the Java method, including its parameter and return types. Calls are checked against the linked extension DEX; a mismatched declaration fails the patch.

## Code at app start

`appEntry { }` adds code that runs once when the process starts, before any activity. `application` is the app's `Application`:

```kotlin
appEntry { call(AdBlocker.init, application) }
```

The engine collects every patch's `appEntry` code in one static method and calls it first in `onCreate()` of the `Application` the manifest names once all patches have run. A patch that changes `<application android:name>` therefore needs no ordering against the patches that use `appEntry`. When that class does not override `onCreate()`, the engine adds an override that calls the inherited one, and clears `final` on the inherited method. A patch using `appEntry` fails when the manifest names no `Application`. The run fails when a later patch leaves the manifest without one, or when the inherited `onCreate()` is static.

## Points

```kotlin
val menuInsert = buildMenu
    .point { checkCast(menuBuilderClass.descriptor) }
    .captureAs("builder")
    .previous { resultOf(Type.ArrayList) }
    .captureAs("items")
    .next { opcode(Opcode.IF_EQZ) }

menuInsert.before {
    call(addMenuItem, capture("builder"), capture("items"))
}

integrityCheck.point { string("device_verified") }
    .next { opcode(Opcode.MOVE_RESULT) }
    .captureAs("verdict")
    .after { capture("verdict").assign(bool(true)) }
```

> [!WARNING]
> A branch to an instruction skips code inserted just before it. For code that must run on every path, anchor after the paths join and use `after { }`. Method return hooks already cover every path into the return.

Emitting twice at one point stacks the blocks in emission order: `before` blocks run one after another just before the instruction, `after` blocks one after another just after it. The point follows its instruction through every later edit of the method, an entry hook included. Once the body is replaced the point is gone, and using it fails with `the method body was replaced under it`.

`captureArgumentAs("name", n)` captures argument `n` of the invoke at the point, and `writer(n)` walks back to the instruction that wrote it:

```kotlin
val setTitle = bindView.point { invokeVirtual { name("setText") } }

setTitle.captureArgumentAs("view", 0).captureArgumentAs("text", 1).before {
    capture("text").assign(call(Titles.clean, capture("text")))
}

setTitle.writer(1).captureAs("raw").after { call(Titles.record, capture("raw")) }
```

`point.skipWhen { condition }` makes the call at the point conditional: the condition is emitted before it, the call runs only when the condition is false, and a branch that targeted the call lands on the guard. The call's result must be unused. With a toggle, `point.skipWhen(setting)` reads the setting as the condition:

```kotlin
playerLayout.point { invokeVirtual { owner("android.view.ViewStub"); name("inflate") } }
    .skipWhen(Settings.hideAutoplayButton)
```

## Registers

Inserted code uses registers the method does not need at that point and grows the frame when it must. Temporary values share registers after their last use, accounting for branches, loops, and wide values. Entry snapshots stay reserved across the method. A replaced body gets sixteen locals below its parameters; exceeding that simultaneous register requirement fails with `Code exceeded the 16 local registers`, and the logic belongs in an extension.

To pass a value between blocks in the same method, reserve a register:

```kotlin
val seen = loadItems.reserveLocal("seen", Type.Boolean)

loadItems.point { string("sponsored") }.after { local(seen).assign(bool(true)) }
loadItems.after { whenTrue(local(seen)) { call(Telemetry.sponsoredShown) } }
```

Reserving grows the frame by the type's width and zeroes the register at method entry, so a read before any write is zero or null. Temporaries never take a reserved register, and frame growth never stages operands through one. A body replacement discards the reservation; `local(slot)` then fails.

Frame growth widens instruction encodings where possible and stages overflowing operands through dead low registers using moves of the appropriate type. Invokes the 35c format cannot encode become range invokes automatically. When no dead contiguous span is available, these invokes share an additional argument area. Incoming arguments are copied at entry to preserve the body's register layout; the argument area does not overlap hook locals or the incoming window. Growth fails without changing the method if a narrow operand has no safe scratch space, the frame exceeds the DEX limit, or the code cannot be relocated. Registers appear only in the [raw bytecode layer](10_dex.md).

## Redirecting a selected call

```kotlin
container.points("store item") {
    invokeVirtual { name("add"); params(Type.Object); returns(Type.Boolean) }
    argument(1) { field { owner(container.owner) } }
}.single().redirectTo(Ads.hideStoreItem)
```

`redirectTo(ExtMethod)` replaces one invoke with a static extension call, the receiver passed first for an instance call. As with `bytecode.redirectCalls`, the target must exist, be static and accessible, and accept the original argument types. Wide values and range invokes keep their registers; branches and points follow the replacement.

A used return must stay assignable to the original return type; an unused boolean return may become void. `redirectTo` rejects direct calls, constructors, super calls, and special invoke forms. `bytecode.redirectCalls` selects virtual, interface, and static calls in app DEX files and validates all of them before editing.

Next: [Manifest, resources, and files](7_runtime.md).

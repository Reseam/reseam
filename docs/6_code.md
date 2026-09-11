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

`thisObject`, `param(i)`, constants (`int`, `bool`, `string`, `nullObject`), `staticField`, `enumValue`, `newInstance`, and `call(...)` produce values. On a value: `cast`, `field`, `set`, `assign` (overwrite in place), `call` for instance methods, `size`, `get`, `plus`, `minus`. The [reference](12_reference.md#changing-methods) lists them all.

```kotlin
whenEnabled(AppSettings.fasterDownloads) {
    thisObject.set(maxParallelDownloads, int(8))
} otherwise {
    thisObject.set(maxParallelDownloads, int(4))
}
```

`whenTrue`, `whenFalse`, `whenNull`, `whenNotNull`, `whenEqual`, `whenNotEqual`, each with an optional `otherwise { }`. Null and false are both a zero test; `whenEqual` is reference equality. `returnVoid`, `returnValue`, `returnTrue`, `returnFalse`, `returnNull` pick the return instruction from the type.

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

`call(AdBlocker.init, thisObject)` calls a method of an [extension](9_extensions.md); the first reference links its DEX into the app. Static methods are `call(ext, args)`, instance methods `receiver.call(ext, args)`. `ext.implement { }` replaces the extension method's body with emitted code, usually from a [binding](8_bindings.md).

> [!WARNING]
> An `ExtClass` declaration is a promise about the Java. A wrong parameter or return type throws `NoSuchMethodError` in the patched app when the call runs, not at patch time.

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

## Registers

Inserted code uses registers the method does not need at that point and grows the frame when it must. Temporary values share registers after their last use, accounting for branches, loops, and wide values. Entry snapshots stay reserved across the method. A replaced body gets sixteen locals below its parameters; exceeding that simultaneous register requirement fails with `Code exceeded the 16 local registers`, and the logic belongs in an extension.

Frame growth widens instruction encodings where possible and stages overflowing operands through dead low registers using moves of the appropriate type. Invokes the 35c format cannot encode become range invokes automatically. When no dead contiguous span is available, these invokes share an additional argument area. Incoming arguments are copied at entry to preserve the body's register layout; the argument area does not overlap hook locals or the incoming window. Growth fails without changing the method if a narrow operand has no safe scratch space, the frame exceeds the DEX limit, or the code cannot be relocated. Registers appear only in the [raw bytecode layer](10_dex.md).

Next: [Manifest, resources, and files](7_runtime.md).

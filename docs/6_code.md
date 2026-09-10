# Changing methods

Once a target is found, a patch changes it by adding code: when the method starts, right before every return, or instead of the whole body. The code is written as Kotlin calls that describe values and calls; Reseam turns them into Dalvik instructions and picks the registers.

Three words place code: `before`, `after`, `replace`. On a method target they mean entry, every return, and the whole body. On a point they mean just before or just after that instruction.

```kotlin
import app.reseam.patch.after
import app.reseam.patch.before
import app.reseam.patch.replace

execute {
    openMedia.before {
        call(DeletedArchive.stripSecureFlag, thisObject.field(windowLayoutParams))
    }

    searchSubtitleBuilder.after {
        returnValue(call(FollowsYouIndicator.appendFromSession, capture("result"), param(1), param(3)))
    }

    updateParams.replace {
        thisObject.set(maxRequests, int(8))
        returnVoid()
    }
}
```

Inside `after`, `capture("result")` is the value being returned. Inside `replace`, the original body is gone.

> **Pitfall.** The Kotlin inside a code block runs once, at patch time, and each call adds instructions. A Kotlin `if` decides what gets emitted; it does not branch in the app. To branch in the app use `whenTrue` and friends. Writing `if (settingEnabled) returnVoid()` compiles and does the wrong thing.

> **Pitfall.** A value (`param(0)`, the result of `call`) belongs to the block that created it. Passing one into another `before { }` fails with `ValueRef belongs to a different code block`. Read parameters again in the new block.

> **Pitfall.** `after` runs before every return instruction of the method, so the block is emitted once per return. `capture("result")` is only available when the method returns something; in a void method it does not exist. `thisObject` throws in a static method.

Whole-method shortcuts: `alwaysReturn()`, `alwaysReturn(true)`, `alwaysReturn(0)`, `alwaysReturn(1L)`, `alwaysReturn("text")`, `alwaysReturnNull()`, `replaceAllStrings(old, new)`, `replaceAllLiterals(old, new)`.

## Values

Code blocks build values. The engine assigns registers and picks encodings.

| Value | Meaning |
|---|---|
| `thisObject` | The receiver; an error in a static method. |
| `param(i)`, `paramOfType(type)`, `lastParam` | Parameters. |
| `capture("name")` | A value captured by `captureAs` on a point, or `result` in `after`. |
| `int(1)`, `long(1L)`, `bool(true)`, `string("x")`, `nullObject` | Constants. |
| `enumValue(type, "NAME")` | An enum constant. |
| `staticField(field)` | A static field read. |
| `newInstance(type, ctorProto, args...)` | A new object. |
| `call(extMethod, args...)`, `call(methodTarget, args...)` | A static call. |
| `callStatic(owner, name, proto, args...)` | A static call by descriptor. |

On a value:

- `cast(type)`.
- `field(fieldTarget)`, `fieldOfType(type)`, `set(fieldTarget, value)`.
- `assign(value)`: overwrite the value where it lives.
- `call(extMethod, args...)`, `call(methodTarget, args...)`: an instance call. `callVirtual(...)` and `callInterface(...)` take descriptors.
- `size()`, `get(index)` on a `List`; `plus`, `minus` on ints.

## Branches

Kotlin control flow runs while the patch is applied. Branches in the app are spelled out:

```kotlin
whenTrue(handled) {
    returnVoid()
}

whenEnabled(TelegramSettings.boostDownloads) {
    thisObject.set(maxRequests, int(8))
} otherwise {
    thisObject.set(maxRequests, int(4))
}
```

`whenTrue`, `whenFalse`, `whenNull`, `whenNotNull`, `whenEqual(a, b)`, `whenNotEqual(a, b)`. Each accepts a chained `otherwise { }`.

`whenTrue` and `whenNotNull` compile to the same zero test, as do `whenFalse` and `whenNull`: a null reference and a false boolean are both zero. `whenEqual` compares registers, so it is reference equality on objects.

Returns: `returnVoid()`, `returnValue(value)`, `returnTrue()`, `returnFalse()`, `returnNull()`. The return instruction is chosen by the value's type.

## Gates

A toggle gates code at runtime through the settings runtime the bundle ships:

```kotlin
import app.reseam.patch.settings.before
import app.reseam.patch.settings.returnFalseWhen

sendTyping.returnFalseWhen(TelegramSettings.hideTyping)

isPremiumUser.before(TelegramSettings.unlockPremium) {
    whenTrue(call(isUserSelf, param(0))) { returnTrue() }
}
```

- `before(toggle) { }`, `after(toggle) { }` on methods and points wrap the block in `whenEnabled(toggle) { }`.
- `skipWhen(toggle)`: return immediately from a void method.
- `returnTrueWhen(toggle)`, `returnFalseWhen(toggle)`: boolean methods.
- `returnNullWhen(toggle)`: object methods.

All live in `app.reseam.patch.settings`.

## Extension methods

Declare an extension class once and name the methods patches call:

```kotlin
object DeletedArchive : ExtClass("app.reseam.telegram.antidelete.DeletedArchive") {
    val init = static("init", Type.Context)
    val markLocalDelete = static("markLocalDelete", Type.Long, Type.ArrayList)
}
```

`static(name, params..., returns = Type.Void)` and `method(name, params..., returns)` for instance methods. `call(DeletedArchive.init, thisObject)` emits the call; the first reference links the extension DEX into the app. See [Shipping your own code](9_extensions.md).

> **Pitfall.** The declaration is a promise about the Java. A wrong parameter type or return type compiles and then throws `NoSuchMethodError` inside the patched app at the moment the call runs, not at patch time. Match the Java signature exactly, `Type.Object` for `Object` parameters included.

> **Pitfall.** Static methods are called as `call(ext, args)`; instance methods as `receiver.call(ext, args)`. Mixing them up is caught at patch time with a message naming the method.

`extMethod.implement { }` replaces the extension method's body with emitted code. A stub compiled into the extension gets its real body this way, usually from a [binding](8_bindings.md).

## Points

```kotlin
val feedMenuInsert = feedMenuBuilder
    .point { checkCast(feedMenuCreator.descriptor) }
    .captureAs("creator")
    .previous { resultOf(Type.ArrayList) }
    .captureAs("menuList")
    .next { opcode(Opcode.INVOKE_STATIC_RANGE) }
    .next { opcode(Opcode.IF_EQZ) }

feedMenuInsert.before {
    call(feedMenuAddItem, enumValue(MEDIA_OPTION, "DOWNLOAD"), capture("creator"), capture("menuList"), int(label))
}
```

`assign` overwrites a captured value where it lives:

```kotlin
safetyNetHandler.point { string("basicIntegrity") }
    .next { opcode(Opcode.MOVE_RESULT) }
    .captureAs("verdict")
    .after { capture("verdict").assign(bool(true)) }
```

## Registers

Inserted code uses registers the method does not need at that point and grows the frame when it must. Replaced bodies get sixteen locals below the parameters; `outs` is sized from the widest call. Invokes with more than five arguments, or arguments in high registers, become range invokes with the arguments moved into a scratch span. Registers appear only in the [raw bytecode layer](10_dex.md).

> **Pitfall.** A `replace { }` body that needs more than sixteen scratch values fails at patch time with `Code exceeded the 16 local registers`. Move the logic into an [extension](9_extensions.md) and call it.

Next: [Manifest, resources, and files](7_runtime.md).

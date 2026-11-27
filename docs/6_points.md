---
description: Select one instruction inside a method, and capture the values around it.
---

# Pointing at instructions

A method target picks a whole method. A *point* picks one instruction inside it, such as the call that shows an ad, so you can add code right before or after that instruction.

```kotlin
val showBanner = loadFeed.point { invokeVirtual { name("showBanner") } }

execute {
    showBanner.before { returnVoid() }
}
```

## Matching an instruction

`point { }` matches the first instruction in the method that fits every constraint:

| Matcher | Matches |
|---|---|
| `string("text")`, `stringContains("t")` | loading a string |
| `literal(42)` | loading a number |
| `invokeVirtual { }`, `invokeStatic { }`, `invokeInterface { }`, `invokeDirect { }` | a call. The block narrows it with `owner`, `name`, `params`, `returns`. |
| `calls(target)` | a call to exactly this method |
| `field { owner(...); name(...) }` | reading or writing a field |
| `arrayRead { type(Type.Int) }`, `arrayWrite { }` | reading or writing an array element. The block is optional. |
| `newInstance(type)`, `checkCast(type)` | creating or casting an object |
| `resultOf(Type.Boolean)` | reading the result of a call (`move-result`) |
| `opcode(Opcode.IF_EQZ)` | any instruction with this opcode |
| `where { ... }` | your own check on the instruction |

[`PointMatch`](reference_targets.md#pointmatch) lists every matcher.

### Sequences

`then { }` continues a sequence. The point lands on its last step:

```kotlin
val premiumCheck = settingsScreen.point {
    string("premium")
    then { invokeStatic { returns(Type.Boolean) } }
}
```

This matches a string load directly followed by a static call, and points at the call. `then(within = 3) { }` looks at the next three instructions, so up to two others can sit in between.

### Where an argument came from

`argument(n) { }` checks the instruction that produced a call's argument:

```kotlin
val attributionLookup = feedView.point {
    invokeVirtual { name("findViewById") }
    argument(1) { literal(attributionId) }
}
```

This finds the `findViewById` call whose id argument is `attributionId`, even when the number was loaded a few instructions earlier. Every path to the call must load a matching value.

## Moving around

From a point you can step to a nearby instruction:

```kotlin
val premiumGetter = mainActivity
    .method("onCreate")
    .point { string("premium_status") }
    .previous { invokeStatic { returns(Type.Boolean) } }
    .callee()
```

- `previous { }` finds the nearest earlier instruction that matches, one step only.
- `next { }` finds the nearest later one, and can be a sequence.
- `callee()` turns a call into a target for the method it calls. `field()` does the same for a field access.
- `writer(n)` points at the instruction that produced argument `n` of a call, following copies between registers.

`callee()` is how you reach a method that has nothing to find it by, but is called from somewhere that does.

## Capturing values

Code you add at a point often needs a value the app computed nearby. Save it with a name, then read it in a code block:

```kotlin
integrityCheck.point { string("device_verified") }
    .next { opcode(Opcode.MOVE_RESULT) }
    .captureAs("verdict")
    .after { capture("verdict").assign(bool(true)) }
```

- `captureAs("name")` saves the value the instruction writes.
- `captureArgumentAs("name", n)` saves argument `n` of a call. For an instance call, argument 0 is the object the method is called on.
- `capture("name")` reads a saved value in a code block at the same point or a later one.

## Every match

`points { }` selects every matching instruction, in one method or across a `methods { }` target. All of them are found before any change, so a change at one doesn't move the others:

```kotlin
methods("attribution") { literals(attributionId) }
    .points("findViewById") {
        invokeVirtual { name("findViewById") }
        argument(1) { literal(attributionId) }
    }
    .forEach {
        next { resultOf(Type.View) }.captureAs("view").after {
            call(Ads.hideAttribution, capture("view"))
        }
    }
```

`.single()` requires exactly one match, and lists every match when there are more.

## Redirecting one call

`redirectTo` replaces the call at a point with a call to a static method of your [extension](10_extensions.md):

```kotlin
loadFeed.point { invokeVirtual { name("getDeviceId") } }.redirectTo(Identity.deviceId)
```

For an instance call, your method receives the object first, then the arguments. Reseam checks that your method's parameters and return type fit the call. Your method may return a supertype of the call's result, such as `Object` for an obfuscated type. The result is then cast back to the call's type. To redirect every call to a method across the app, use [`bytecode.redirectCalls`](7_changing_code.md#redirecting-a-call).

## Arrays

An array access works like a call: a read is `get(array, index)` and a write is `set(array, index, value)`. So `argument`, `captureArgumentAs`, `writer`, and `redirectTo` work on it, with the array as argument 0, the index as 1, and the value written as 2:

```kotlin
val flag = checkFlags.point {
    arrayRead { type(Type.Boolean) }
    argument(0) { field { name("enabled") } }
    argument(1) { literal(3) }
}
flag.redirectTo(Flags.isEnabled)
```

`Flags.isEnabled` takes `(BooleanArray, Int)` and returns `Boolean`, and the app uses what it returns as the element. A redirected write calls a method taking `(array, index, value)`.

`arrayRead { type(...) }` checks the element type. An object array's type comes from where the array was written, such as a field, a parameter, or a call's result. When Reseam can't tell, the point doesn't match and a redirect fails.

## Points follow their instruction

Adding code anywhere in the method doesn't break a point: it keeps naming the same instruction. Replacing the whole method body does. Using a point after that fails with `the method body was replaced under it`.

Next: [Changing code](7_changing_code.md).

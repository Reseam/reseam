---
description: Select one instruction inside a method, and capture the values around it.
---

# Pointing at instructions

A method target picks a whole method. A *point* picks one instruction inside it, such as the call that shows an ad, so you can add code right before or after it.

```kotlin
val showBanner = loadFeed.point { invokeVirtual { name("showBanner") } }

showBanner.before { returnVoid() }
```

## Matching an instruction

`point { }` matches the first instruction that fits:

| Matcher | Matches |
|---|---|
| `string("text")`, `stringContains("t")` | loading a string |
| `literal(42)` | loading a number |
| `invokeVirtual { }`, `invokeStatic { }`, `invokeInterface { }`, `invokeDirect { }` | a call; the block narrows it with `owner`, `name`, `params`, `returns` |
| `field { owner(...); name(...) }` | reading or writing a field |
| `newInstance(type)`, `checkCast(type)` | creating or casting an object |
| `opcode(Opcode.IF_EQZ)` | any instruction with this opcode |
| `where { ... }` | your own check on the instruction |

`then { }` continues a sequence: `point { string("premium") }.then { invokeStatic { } }` matches a string load followed directly by a static call, and points at the call. `then(within = 3) { }` looks at the next three instructions, so up to two can sit in between.

## Moving around

From a point you can step to a nearby instruction:

```kotlin
val premiumCheck = mainActivity
    .method("onCreate") { strings("premium_status") }
    .point { string("premium_status") }
    .previous { invokeStatic { returns(Type.Boolean) } }
    .callee()
```

- `previous { }` and `next { }` find the nearest earlier or later match.
- `callee()` turns a call into a target for the method it calls. `field()` does the same for a field access.

## Capturing values

Code you add at a point often needs a value the app computed there:

```kotlin
integrityCheck.point { string("device_verified") }
    .next { opcode(Opcode.MOVE_RESULT) }
    .captureAs("verdict")
    .after { capture("verdict").assign(bool(true)) }
```

- `captureAs("name")` saves the value the instruction writes.
- `captureArgumentAs("name", n)` saves argument `n` of a call. For an instance call, argument 0 is the object the method is called on.
- `writer(n)` points at the instruction that produced argument `n`.

Read a saved value with `capture("name")` in a code block.

## Every match

`points { }` selects every matching instruction, in one method or across a `methods { }` target:

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

`argument(n) { }` checks where an argument's value came from. `.single()` requires exactly one match.

## Points follow their instruction

Adding code anywhere in the method doesn't break a point: it keeps naming the same instruction. Replacing the whole method body does, and using the point after that fails with `the method body was replaced under it`.

Next: [Changing code](8_changing_code.md).

---
description: Add code at the start of a method, before it returns, around an instruction, or instead of it.
---

# Changing code

Once a target is found, you add code to it. You write the code as Kotlin calls, and Reseam turns them into Android bytecode, choosing the registers for you.

```kotlin
execute {
    loadFeed.before {
        AdBlocker.onFeedLoad(thisObject.field(feedItems))
    }

    buildTitle.after {
        returnValue(Badges.append(result))
    }

    configureDownloads.replace {
        thisObject.set(maxParallelDownloads, int(8))
        returnVoid()
    }
}
```

## Where code goes

| On a method | |
|---|---|
| `before { }` | at the start |
| `after { }` | before every return. `result` is the value being returned. |
| `replace { }` | instead of the whole body |

On a [point](6_points.md), `before { }` and `after { }` place code just before or just after that instruction.

For the common cases there are shortcuts: `alwaysReturn()`, `alwaysReturn(true)`, `alwaysReturn(0)`, `alwaysReturn("text")`, and `alwaysReturnNull()` replace the body with a constant return.

> [!WARNING]
> The Kotlin in a code block runs once, while patching, and each call adds instructions to the app. A Kotlin `if` chooses what to add. It is not a check in the app. Use `whenTrue` and the other [branches](#branches-and-returns) for that.

## Values

Every value in a code block is a `ValueRef`:

| | |
|---|---|
| `thisObject`, `param(i)`, `lastParam` | the object the method runs on, and its parameters |
| `int(1)`, `long(1)`, `bool(true)`, `string("x")`, `nullObject` | constants |
| `AdBlocker.init(context)`, `call(target, args)` | call a static method of your extension, or of the app |
| `value.call(method, args)` | call an instance method on an object |
| `value.field(f)`, `staticField(f)` | read a field |
| `value.set(f, v)`, `setStatic(f, v)` | write a field |
| `value[index]`, `value[index] = v`, `value.size()` | read or write an element of an array or `List`, or get its size |
| `value.cast(type)`, `newInstance(type)` | cast, or create an object |
| `capture("name")`, `value.assign(v)` | read a captured value, or overwrite a value |

[API: Code blocks](reference_code.md#codescope) lists everything.

A value belongs to the block that created it. Using it in another block fails with `ValueRef belongs to a different code block`. To pass a value between blocks, use a [reserved local](#passing-a-value-between-blocks).

Reading a field gives you a copy. To change the field, write it back with `set`. `assign` on a value read from a field fails.

## Branches and returns

```kotlin
isPremium.replace {
    whenTrue(call(isCurrentUser, param(0))) {
        returnTrue()
    } otherwise {
        returnFalse()
    }
}
```

| Branch | Runs the block in the app when |
|---|---|
| `whenTrue(v)`, `whenFalse(v)` | `v` is true or nonzero, or false or zero. For an object, non-null or null. |
| `whenNull(v)`, `whenNotNull(v)` | the object is null, or not |
| `whenEqual(a, b)`, `whenNotEqual(a, b)` | numbers by value, objects by identity |
| `whenInstanceOf(v, type)` | the object is of this type |

Each takes an optional `otherwise { }`. To return, use `returnVoid()`, `returnValue(v)`, `returnTrue()`, `returnFalse()`, or `returnNull()`. A block that doesn't return falls through to the method's own code.

## Gates

A gate checks a [setting](11_settings.md) while the app runs:

```kotlin
gate(AppSettings.hideAds) {
    showAd.alwaysReturn()
    adSlot.alwaysReturnNull()
    loadFeed.after { thisObject.set(adCount, int(0)) }
}
```

The hooks inside `gate(setting) { }` only take effect while the toggle is on: `before { }`, `after { }`, and `alwaysReturn…`, on methods and points. On a point, `skip()` skips that one call. While the toggle is off, the app's own code runs unchanged.

Inside a code block, `whenEnabled` branches on a toggle, and `valueOf` reads any setting:

```kotlin
configureDownloads.after {
    whenEnabled(AppSettings.fasterDownloads) {
        thisObject.set(maxParallelDownloads, int(8))
    }
}
```

## Calling your own code

Anything longer than a few lines is easier to write as normal Kotlin in an [extension](10_extensions.md). Mark what patches call with `@Export`, and call it through the binding the build generates:

```kotlin
import app.example.ads.bindings.AdBlocker

loadFeed.before { AdBlocker.onFeedLoad(thisObject.field(feedItems)) }
```

### Redirecting a call

Instead of adding code, you can send the app's calls to your own method. `point.redirectTo(extMethod)` does it for one call. `bytecode.redirectCalls` does it for every call to a method across the app:

```kotlin
bytecode.redirectCalls("android.telephony.TelephonyManager", "getDeviceId", Identity.deviceId)
```

Your method takes the same parameters as the original, with the object first for an instance method, and returns the same type. Calls made from your own extensions keep going to the original, so your method can still call it.

### Adding a method to an app class

To give an app class a method of its own, add it with its body. Other code can then call it:

```kotlin
val seekHelper = playerClass.addMethod("reseam_seekTo", "(J)Z") {
    returnValue(thisObject.call(seek, param(0)))
}
```

If the class already has the method, its body is replaced, so patches that share a helper can each add it. To override a superclass method, add it with the superclass's name and prototype, and use `thisObject.callSuper(...)` to call the original.

## Code at app start

`appEntry { }` adds code that runs once when the app's process starts, before any screen opens. `application` is the app's `Application` object:

```kotlin
appEntry { AdBlocker.init(application) }
```

The app must name an `Application` class in its manifest. The code lands in whichever class the manifest names after every patch has run, so a patch that swaps the class doesn't strand it.

## Passing a value between blocks

To set a value in one place and read it in another, within the same method, reserve a local:

```kotlin
val seen = loadItems.reserveLocal("seen", Type.Boolean)

loadItems.point { string("sponsored") }.after { local(seen).assign(bool(true)) }
loadItems.after { whenTrue(local(seen)) { Telemetry.sponsoredShown() } }
```

It holds zero or null when the method starts.

## Things to know

- A point's `before { }` also runs when a branch jumps to that instruction. Its `after { }` doesn't: a branch that jumps to the next instruction skips it. For code that must run on every path, use `before { }` on a later point, or the method's `after { }`.
- A method's `after { }` saves the parameters and `thisObject` it reads at entry, because the method may reuse their registers for other values.
- Several hooks on one method or point stack up, in the order patches add them.
- Code that needs more registers than the instructions around it can reach fails with `... needs a free register at or below v...`. Move the logic into an extension and call it.
- Access is checked: calling a private method of another class fails while patching, not in the app.

Next: [Manifest, resources, and files](8_app_files.md).

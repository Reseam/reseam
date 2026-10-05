---
description: Add code at the start of a method, before it returns, around an instruction, or replace it.
---

# Changing code

Once a target is found, you add code to it. You write the code as Kotlin calls; Reseam turns them into Android bytecode and handles the registers.

```kotlin
execute {
    loadFeed.before {
        call(AdBlocker.onFeedLoad, thisObject.field(feedItems))
    }

    buildTitle.after {
        returnValue(call(Badges.append, capture("result")))
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
| `after { }` | before every return; `capture("result")` is the value being returned |
| `replace { }` | instead of the whole body |

On a [point](7_points.md), `before { }` and `after { }` place code just before or just after that instruction.

Shortcuts for common cases: `alwaysReturn(true)`, `alwaysReturnNull()`, `replaceAllStrings(old, new)`, `replaceAllLiterals(old, new)`.

> [!WARNING]
> The Kotlin in a code block runs once, while patching, and each call adds instructions. A Kotlin `if` chooses what to add; it is not a branch in the app. Use `whenTrue` and the other branches below.

## Values

| | |
|---|---|
| `thisObject`, `param(i)`, `lastParam` | the object and the method's parameters |
| `int(1)`, `bool(true)`, `string("x")`, `nullObject` | constants |
| `call(extMethod, args)`, `value.call(...)` | call a method and use its result |
| `value.field(f)`, `staticField(f)` | read a field |
| `value.set(f, v)`, `setStatic(f, v)` | write a field |
| `value.cast(type)`, `newInstance(type)` | cast, or create an object |
| `capture("name")`, `value.assign(v)` | read a captured value, or overwrite it |

The [reference](reference.md#changing-methods) lists everything. A value belongs to the block that created it; using it in another block fails.

## Branches and returns

```kotlin
whenTrue(call(isCurrentUser, param(0))) {
    returnTrue()
} otherwise {
    returnFalse()
}
```

`whenTrue`, `whenFalse`, `whenNull`, `whenNotNull`, `whenEqual`, `whenNotEqual`, and `whenInstanceOf` each take an optional `otherwise { }`. `returnVoid`, `returnValue`, `returnTrue`, `returnFalse`, and `returnNull` return from the method.

## Gates

A gate checks a [setting](5_settings.md) while the app runs:

```kotlin
gate(AppSettings.hideAds) {
    showAd.alwaysReturn()
    adSlot.alwaysReturnNull()
    loadFeed.after { thisObject.set(adCount, int(0)) }
}

whenEnabled(AppSettings.fasterDownloads) {
    thisObject.set(maxParallelDownloads, int(8))
}
```

Hooks written inside `gate(setting) { }` only take effect while the setting is on: `before { }`, `after { }`, and `alwaysReturn…`, on methods and points. On a point, `skip()` skips that one call. Inside a code block, `whenEnabled` branches on a setting.

## Calling your own code

For anything longer than a few lines, write Java in an [extension](11_extensions.md) and call it:

```kotlin
object AdBlocker : ExtClass("app.example.ext.AdBlocker") {
    val onFeedLoad by static(Type.List)
}

loadFeed.before { call(AdBlocker.onFeedLoad, thisObject.field(feedItems)) }
```

### Redirecting a call

`point.redirectTo(extMethod)` replaces one call with a call to your static method. For an instance call, the object comes first. `bytecode.redirectCalls(owner, name, extMethod)` does the same for every call to a method anywhere in the app:

```kotlin
bytecode.redirectCalls("android.telephony.TelephonyManager", "getDeviceId", Identity.deviceId)
```

Calls made from your own extension code keep going to the original, so your method can still call it.

## Code at app start

`appEntry { }` adds code that runs once when the app's process starts, before any screen opens. `application` is the app's `Application` object:

```kotlin
appEntry { call(AdBlocker.init, application) }
```

The app must name an `Application` class in its manifest.

## Passing a value between blocks

To set a value in one place and read it in another within the same method, reserve a local:

```kotlin
val seen = loadItems.reserveLocal("seen", Type.Boolean)

loadItems.point { string("sponsored") }.after { local(seen).assign(bool(true)) }
loadItems.after { whenTrue(local(seen)) { call(Telemetry.sponsoredShown) } }
```

It starts as zero or null when the method begins.

## Things to know

- A point's `before { }` also runs when a branch jumps to that instruction. Its `after { }` does not: a branch that jumps straight to the next instruction skips it. For code that must run on every path, use `before { }` on a later point, or the method's `after { }`.
- Added code that needs more registers than the surrounding instructions can address fails with `Cannot allocate ... registers`. Move that logic into an [extension](11_extensions.md).
- Reading a field gives a copy. Write it back with `set`, not `assign`.
- Access is checked: calling a private method of another class fails while patching.

Next: [Manifest, resources, and files](9_app_files.md).

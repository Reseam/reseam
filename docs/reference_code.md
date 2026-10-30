---
title: "API: Code blocks"
description: Adding code to methods, the values code blocks work with, extension bindings, and app-wide bytecode changes.
---

# API: Code blocks

How a patch adds code to the app. The guide page is [Changing code](7_changing_code.md). Everything here is in `app.reseam.patch`.

A code block is Kotlin that runs while patching. Each call in it adds instructions to the app, and Reseam chooses the registers. A Kotlin `if` in a block decides what to add. To branch inside the app, use `whenTrue` and the other `when…` functions.

## Hooks

### On a method

| Function | Description |
|---|---|
| `MethodTarget.before(block: CodeScope.() -> Unit)` | Adds code at the start of the method. |
| `MethodTarget.after(block: CodeScope.() -> Unit)` | Adds code before every return. In a method that returns a value, `result` is that value. The parameters and `thisObject` the block reads are saved at entry, because the method may reuse their registers. |
| `MethodTarget.replace(block: CodeScope.() -> Unit)` | Replaces the whole body. Points in the method stop working afterwards. |
| `MethodTarget.alwaysReturn()` | Replaces the body with `return`. |
| `MethodTarget.alwaysReturn(value: Boolean)`, `(value: Int)`, `(value: Long)`, `(value: String)` | Replaces the body with `return value`. The value must suit the return type. |
| `MethodTarget.alwaysReturnNull()` | Replaces the body with `return null`. |
| `MethodTarget.reserveLocal(name: String, type: String): MethodLocal` | Reserves a register that every code block in this method can use, as `local(slot)`. It holds zero or null when the method starts. |

Several hooks on one method stack up in the order patches add them.

### At a point

| Function | Description |
|---|---|
| `PointTarget.before(block: CodeScope.() -> Unit)` | Adds code just before the instruction. It also runs when a branch jumps to the instruction. |
| `PointTarget.after(block: CodeScope.() -> Unit)` | Adds code just after the instruction. It only runs when the instruction falls through to the next one, so it fails on a `return`, `throw`, or `goto`. |
| `PointTarget.skipWhen(condition: CodeScope.() -> ValueRef)` | Runs the call at the point only when `condition` is false. The call's result must be unused, since a skipped call has none. |

### At app start

| Function | Description |
|---|---|
| `appEntry(block: AppEntryScope.() -> Unit)` | Adds code that runs once when the app's process starts, first in `onCreate()` of the `Application` class. The engine places it after every patch has run, so it lands in whichever class the final manifest names. Fails when the manifest names no `Application`. |

### `AppEntryScope`

The receiver of `appEntry { }`: a [`CodeScope`](#codescope) with one more member, `application: ValueRef`, the app's `Application` object. It has no `thisObject`.

### `MethodLocal`

What `reserveLocal` returns. Pass it to `local(slot)` in a code block of the same method.

| Member | Description |
|---|---|
| `name: String` | The name you gave it, for error messages. |
| `type: String` | Its type. A `long` or `double` takes two registers. |

Replacing the method's body ends the local.

## Values

### `CodeScope`

The receiver of every code block.

**Reading the method's values**

| Member | Description |
|---|---|
| `thisObject: ValueRef` | The object the method runs on. Fails in a static method. |
| `param(index: Int): ValueRef` | A parameter, counting from 0. The object the method runs on isn't counted. |
| `paramOfType(type: String): ValueRef` | The first parameter of this type. Fails when there is none. |
| `lastParam: ValueRef` | The last parameter. |
| `result: ValueRef` | The value being returned, in a method's `after { }`. |
| `capture(name: String): ValueRef` | A value a point saved with `captureAs` or `captureArgumentAs`. |
| `local(slot: MethodLocal): ValueRef` | A register reserved with `reserveLocal`. |

**Constants**

| Member | Description |
|---|---|
| `int(value: Int): ValueRef` | An `int`. |
| `long(value: Long): ValueRef` | A `long`. |
| `bool(value: Boolean): ValueRef` | A `boolean`. |
| `string(value: String): ValueRef` | A `String`. |
| `nullObject: ValueRef` | `null`. |
| `enumValue(type: String, name: String): ValueRef` | The enum constant in the field called `name`. For an obfuscated enum, use `staticField(klass.enumConstant(…))`. |

**Fields, objects, and calls**

| Member | Description |
|---|---|
| `staticField(field: FieldTarget)`, `staticField(field: FieldRef): ValueRef` | Reads a static field. |
| `setStatic(field: FieldTarget, value: ValueRef)`, `setStatic(field: FieldRef, value: ValueRef)` | Writes a static field. |
| `newInstance(type: String, ctorProto: String = "()V", vararg args: ValueRef): ValueRef` | Creates an object, calling the constructor with this prototype. |
| `call(method: ExtMethod, vararg args: ValueRef): ValueRef` | Calls a static extension method. Fails when the method doesn't exist or the argument count is wrong. Call an instance method on its object instead: `value.call(method)`. |
| `ExtMethod.invoke(vararg args: ValueRef): ValueRef` | The same as `call`, written as a call: `AdBlocker.init(application)`. |
| `call(method: MethodTarget, vararg args: ValueRef): ValueRef` | Calls a static app method. Call an instance method on its object: `value.call(method)`. |
| `callStatic(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef` | Calls any static method by name, including platform methods. |

**Branches**

Each takes a block and returns an [`Otherwise`](#otherwise), so you can add `otherwise { }`.

| Member | Runs the block in the app when |
|---|---|
| `whenTrue(value: ValueRef, block)` | `value` is nonzero, `true`, or a non-null object. |
| `whenFalse(value: ValueRef, block)` | `value` is zero, `false`, or null. |
| `whenNull(value: ValueRef, block)` | `value` is null. Only for objects. |
| `whenNotNull(value: ValueRef, block)` | `value` is not null. Only for objects. |
| `whenEqual(left: ValueRef, right: ValueRef, block)` | the values are equal: numbers by value, objects by identity. NaN is never equal, and `0.0` equals `-0.0`. |
| `whenNotEqual(left: ValueRef, right: ValueRef, block)` | the values are not equal, by the same rules. |
| `whenInstanceOf(value: ValueRef, type: String, block)` | `value` is an instance of `type`. Null isn't an instance of anything. |

**Returning**

| Member | Description |
|---|---|
| `returnVoid()` | Returns from a `void` method. |
| `returnValue(value: ValueRef)` | Returns `value`. |
| `returnTrue()`, `returnFalse()` | Returns a `boolean`. |
| `returnNull()` | Returns `null`. |

Settings add more members to `CodeScope`: [`whenEnabled`](reference_settings.md#gates), [`valueOf`](reference_settings.md#reading-settings-in-code-blocks), [`openSettings`](reference_settings.md#the-app-runtime-in-code-blocks), and [`addReactPackage`](reference_settings.md#the-app-runtime-in-code-blocks).

### `ValueRef`

A value inside a code block. A value belongs to the block that created it, and using it in another block fails.

| Member | Description |
|---|---|
| `type: String` | Its type's descriptor. |
| `cast(type: String): ValueRef` | The value cast to `type`. |
| `field(field: FieldTarget)`, `field(ref: FieldRef): ValueRef` | Reads an instance field of this object. |
| `fieldOfType(type: String): ValueRef` | Reads the one instance field of this type on the object's class. |
| `set(field: FieldTarget, value: ValueRef)`, `set(ref: FieldRef, value: ValueRef)` | Writes an instance field of this object. |
| `assign(value: ValueRef)` | Overwrites this value in place: a parameter, a capture, a local, or an earlier result. A value read from a field is a copy, so assigning it fails. Write the field with `set` instead. |
| `call(method: ExtMethod, vararg args: ValueRef): ValueRef` | Calls an instance method of an extension class on this object. |
| `call(method: MethodTarget, vararg args: ValueRef): ValueRef` | Calls an instance method of the app on this object. |
| `callVirtual(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef` | Calls a method on this object by name. |
| `callInterface(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef` | Calls an interface method on this object by name. |
| `callSuper(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef` | Calls the superclass's version of a method, as `super.name(args)` does in a subclass of `owner`. Use it in a method you add with `addMethod` to override one. |
| `size(): ValueRef` | `List.size()` of this value. |
| `get(index: ValueRef): ValueRef` | `List.get(index)`, written `value[index]`. |
| `plus(other: ValueRef): ValueRef`, `minus(other: ValueRef): ValueRef` | Adds or subtracts two numbers, written `a + b` and `a - b`. |

### `Otherwise`

What a `when…` function returns. `otherwise(block: CodeScope.() -> Unit)` adds the other branch: `whenNull(x) { … } otherwise { … }`.

## Extension bindings

The build generates these for the classes of your [extensions](10_extensions.md). You use them; you rarely write them.

### `ExtClass`

A class your bundle ships as an extension. The generated binding is an `object` that extends it, with one property per export:

```kotlin
object AdBlocker : ExtClass("app.example.ads.AdBlocker") {
    val isAd by static(Type.Object, returns = Type.Boolean)
}
```

| Member | Description |
|---|---|
| `ExtClass(name: String)` | The class, by dotted name or descriptor. |
| `descriptor: String` | Its descriptor. |
| `target: ClassTarget` | The class as a target, for example to make an app class extend it. |
| `static(vararg params: String, returns: String = Type.Void, name: String? = null): MemberDelegate<ExtMethod>` | Declares a static method, named after the property unless `name` is given. |
| `method(vararg params: String, returns: String = Type.Void, name: String? = null): MemberDelegate<ExtMethod>` | Declares an instance method. |
| `field(type: String, name: String? = null): MemberDelegate<FieldTarget>` | Declares a field. |

The engine adds the extension's code to the app the first time a patch refers to one of its members.

### `ExtMethod`

A method of an extension.

| Member | Description |
|---|---|
| `owner: String` | The extension class's descriptor. |
| `name: String` | The method's name. |
| `proto: String` | Its prototype. |
| `isStatic: Boolean` | Whether it is static. |
| `ref: MethodRef` | A reference to it. |
| `target: MethodTarget` | The method as a target, so you can hook it like an app method. |
| `implement(block: CodeScope.() -> Unit)` | Replaces the method's body with emitted code. An `external` function gets the body it lacks. |

`implement` lets an extension leave a function for a patch to fill in. The extension declares it `external`, and calls it like any other function:

```kotlin
@Export
external fun videoUrl(media: Any?): String?
```

The patch writes the body, with code only a patch can build, such as a [binding](12_bindings.md):

```kotlin
Media.videoUrl.implement { returnValue(media.member("videoUrl", param(0))) }
```

The run fails when the patched app can call an `external` function that no applied patch implemented.

### `MemberDelegate`

What `static`, `method`, and `field` return: a property delegate that names the member after the property. You never use it directly.

## App-wide changes

### `BytecodeScope`

The `bytecode` of a running patch: every class in the app, and changes across all of them.

| Member | Description |
|---|---|
| `classes: List<DexClass>` | Every class in the app. Prefer the functions below, which use indexes, to filtering this list. |
| `findClass(name: String): DexClass?` | A class by its name. |
| `classesExtending(type: String): List<DexClass>` | Every class with `type` anywhere above it. |
| `classesImplementing(type: String): List<DexClass>` | Every class that declares `type` as an interface itself. |
| `replaceAllStrings(old: String, new: String): Int` | Replaces every string constant that is exactly `old`. Returns how many changed. |
| `replaceStringsContaining(substring: String, transform: (String) -> String?): Int` | Rewrites every string constant containing `substring`. `transform` returns the new string, or null to leave one as it is. Only methods holding a match are visited. |
| `replaceStringsStartingWith(prefix: String, transform: (String) -> String?): Int` | The same, for constants that start with `prefix`. |
| `redirectCalls(from: MethodRef, to: ExtMethod): Int` | Turns every call to `from` in the app into a call to the static `to`, with the object first for an instance method. Calls through `super`, constructor calls, and calls from your extensions are left alone, so `to` can still call `from`. Returns how many calls changed. |
| `redirectCalls(owner: String, name: String, to: ExtMethod): Int` | The same, with `from` worked out from `to`: the method `owner.name` with `to`'s parameters (minus the object, for an instance method) and return type. |

---
title: "API: Targets and points"
description: Finding methods, classes, fields, instructions, and values inside obfuscated objects.
---

# API: Targets and points

How a patch finds code in the app. The guide pages are [Finding code](5_finding_code.md), [Pointing at instructions](6_points.md), and [Reading obfuscated objects](12_bindings.md). Everything here is in `app.reseam.patch`.

Every target is searched for when a patch first uses it, not when you declare it. So you can declare targets as top-level values, and one target's query can read another target.

## Methods

### Method target functions

| Function | Description |
|---|---|
| `method(debugName: String? = null, block: MethodQuery.() -> Unit): MethodTarget` | The one method matching the query. Several matches fail, unless the query ranks them or calls `first()`. |
| `methods(debugName: String? = null, block: MethodQuery.() -> Unit): MethodsTarget` | Every method matching the query, best ranked first. |
| `methodTarget(debugName: String, resolve: PatchRuntime.() -> Method): MethodTarget` | A method you find yourself, with the whole app available. Use it when no query can say what you need. |
| `ClassTarget.method(name: String, inherited: Boolean = false, block: MethodQuery.() -> Unit = {}): MethodTarget` | The method of this class called `name`. The block narrows overloads. `inherited = true` also looks at methods the class inherits from other app classes. |
| `ClassTarget.methods(debugName: String? = null, inherited: Boolean = false, block: MethodQuery.() -> Unit): MethodsTarget` | Every method of this class matching the query. |
| `ClassTarget.addMethod(name: String, proto: String, accessFlags: Int = …, body: CodeScope.() -> Unit): MethodTarget` | Gives the class a method with this body. If the class already declares a method with this name and prototype, that method's body is replaced instead. So patches that share a helper can each add it, and the class ends up with one copy. The flags default to public final, or public on an interface. |

### `MethodTarget`

One method. Reading a property resolves the target, so it only works while a patch is running.

| Member | Description |
|---|---|
| `method: Method` | The method, for [raw bytecode](reference_dex.md#method) work. |
| `owner: String` | The descriptor of the class that declares it. |
| `name: String` | Its name in this version of the app. |
| `proto: String` | Its prototype, such as `(Ljava/lang/String;)Z`. |
| `returnType: String` | Its return type's descriptor. |
| `parameterTypes: List<String>` | Its parameter types' descriptors. |
| `descriptor: String` | `owner->name(params)returns`. |
| `ref: MethodRef` | A [`MethodRef`](reference_dex.md#methodref) to it. |

Code blocks and points attach to a `MethodTarget`. See [API: Code blocks](reference_code.md#hooks).

### `MethodsTarget`

Several methods.

| Member | Description |
|---|---|
| `all: List<MethodTarget>` | Every match, best ranked first. |
| `forEach(block: MethodTarget.() -> Unit)` | Runs the block with each match as its receiver. |
| `single(predicate: MethodTarget.() -> Boolean = { true }): MethodTarget` | A target for the one match the predicate accepts. It fails, listing every candidate, unless exactly one does. Declaring it doesn't search, so it can be a top-level value. |

### `MethodQuery`

The receiver of `method { }` and `methods { }`. Every constraint must hold.

| Member | Matches methods that |
|---|---|
| `name(value: String)` | are called exactly this. Only useful for names obfuscation keeps, such as `onCreate`. |
| `strings(vararg values: String)` | load each of these exact strings. |
| `stringsStartingWith(prefix: String)` | load a string that starts with `prefix`. |
| `stringsContaining(vararg parts: String)` | load, for each part, a string that contains it. |
| `literals(vararg values: Long)` | use each of these number constants. |
| `returns(type: String)` | return this type. |
| `params(vararg types: String)` | take exactly these parameters, in order. `params()` means none. |
| `param(index: Int, type: String)` | take this type at this position. |
| `hasParam(type: String)` | take a parameter of this type anywhere. |
| `paramCount(count: Int)` | take this many parameters. |
| `flags(mask: Int)` | carry every [access flag](reference_dex.md#accessflags) in the mask. `flags(AccessFlags.PUBLIC or AccessFlags.STATIC)` needs both. |
| `inClass(target: ClassTarget, inherited: Boolean = false)` | are declared in this class. `inherited = true` also accepts methods it inherits from app classes it extends. |
| `calls(target: MethodTarget)` | call this method. |
| `calls(ref: MethodRef)` | call exactly this reference, including a platform method such as `android.app.Activity.finish`. |
| `calls(block: MethodRefMatch.() -> Unit)` | call a method matching the block. |
| `calledBy(target: MethodTarget)` | are called by this method. |
| `accesses(block: FieldRefMatch.() -> Unit)` | read or write a field matching the block. |
| `callsMethod(predicate: MethodRef.() -> Boolean)` | call any method the predicate accepts, in the app or the platform. |
| `opcode(vararg opcodes: Opcode)` | contain each of these instructions somewhere, in any order. |
| `opcodeSequence(vararg opcodes: Opcode?)` | contain these instructions back to back. `null` matches any one instruction. |
| `custom(predicate: Method.() -> Boolean)` | pass your own check. |
| `rankBy(label: String, block: MethodRankScope.() -> Int)` | Not a constraint: scores each match. The highest score wins. A tie still fails. |
| `first()` | Not a constraint: when several methods match, takes the best ranked one instead of failing. |
| `firstInAppOrder()` | Not a constraint: like `first()`, but ties go to the method the app's class loader meets first. It compares the DEX file, then the class, then the method's place in its class, direct methods first. |
| `includeExtensions()` | Not a constraint: also searches the classes your bundle's extensions add. By default, queries only search the app's own code. |

How the search runs: the most selective constraint finds the first candidates, through indexes of strings, numbers, calls, and field accesses. The rest filter them. `custom` and `opcodeSequence` only filter, so pair them with something selective, or the query checks every method in the app.

### `MethodRankScope`

The receiver of `rankBy` on a method query. It has the members of [`RankScope`](#rankscope), plus:

| Member | Description |
|---|---|
| `method: Method` | The candidate. |
| `paramCount: Int` | How many parameters it takes. |
| `callSitesFollowedByCast(type: String, lookAhead: Int = 40): Int` | How many of its call sites cast the result to `type` within the next `lookAhead` instructions. |

### `RankScope`

Members every ranking block has.

| Member | Description |
|---|---|
| `type: String` | The candidate's class descriptor. |
| `methods(proto: String): List<Method>` | The candidate class's methods with this prototype. |
| `zeroArgListGetters(): Int` | How many methods of the candidate class take nothing and return a `List`. |

## Classes

### Class target functions

| Function | Description |
|---|---|
| `klass(name: String): ClassTarget` | A class by its name, dotted or as a descriptor. Use it for names obfuscation keeps, such as activities named in the manifest. |
| `klass(debugName: String, block: ClassQuery.() -> Unit): ClassTarget` | The one class matching the query. |
| `classTarget(debugName: String, resolve: PatchRuntime.() -> DexClass): ClassTarget` | A class you find yourself. |

### `ClassTarget`

| Member | Description |
|---|---|
| `classDef: DexClass` | The class, for [raw bytecode](reference_dex.md#dexclass) work. |
| `descriptor: String` | Its descriptor in this version of the app. |

A class target also narrows other lookups: `method`, `methods`, `addMethod`, `field`, `enumConstant`, and `fieldOfType`.

### `ClassQuery`

The receiver of `klass(debugName) { }`.

| Member | Matches classes that |
|---|---|
| `strings(vararg values: String)` | load each of these strings in their methods. |
| `hasInstanceField(type: String)` | have an instance field of this type. Repeat it to require several. |
| `enumConstants(vararg names: String)` | are enums with constants of these names, as `name()` returns them. Obfuscation renames the constants' fields but keeps these names. |
| `extends(type: String)` | have `type` anywhere above them. |
| `implements(type: String)` | declare `type` as an interface themselves. |
| `sourceFile(name: String)` | record this source file name. Many obfuscators keep it. |
| `custom(predicate: DexClass.() -> Boolean)` | pass your own check. Pair it with a selective constraint. |
| `rankBy(label: String, block: ClassRankScope.() -> Int)` | Scores each match; the highest wins. |
| `first()` | Takes the best ranked match instead of failing on several. |
| `firstInAppOrder()` | Like `first()`, but ties go to the class in the earliest DEX file, then the earliest in it. |
| `includeExtensions()` | Also searches the classes your bundle's extensions add. |

### `ClassRankScope`

The receiver of `rankBy` on a class query. It has the members of [`RankScope`](#rankscope), plus `classDef: DexClass`, the candidate.

## Fields

### Field target functions

| Function | Description |
|---|---|
| `ClassTarget.field(name: String): FieldTarget` | The field of this class called `name`. |
| `ClassTarget.fieldOfType(type: String): FieldTarget` | The one instance field of this class with this type. |
| `ClassTarget.enumConstant(name: String): FieldTarget` | The field that holds this enum's constant whose `name()` is `name`, even when obfuscation renamed the field. |
| `MethodTarget.field(debugName: String? = null, block: FieldRefMatch.() -> Unit): FieldTarget` | The one field this method reads or writes that matches the block. |
| `field(owner: String, name: String, type: String): FieldTarget` | A field reference without a lookup, for fields on classes your bundle adds. |
| `fieldTarget(debugName: String, resolve: PatchRuntime.() -> FieldRef): FieldTarget` | A field you find yourself. |

### `FieldTarget`

| Member | Description |
|---|---|
| `ref: FieldRef` | A [`FieldRef`](reference_dex.md#fieldref) to it. |
| `owner: String` | The descriptor of the class that declares it. |
| `name: String` | Its name in this version of the app. |
| `type: String` | Its type's descriptor. |

## Matching references

### `MethodRefMatch`

Describes a method that is called, in `calls { }` and in point matchers such as `invokeVirtual { }`.

| Member | Matches calls whose method |
|---|---|
| `owner(type: String)` | is declared on exactly this class. |
| `ownerAssignableTo(type: String)` | is declared on this class or a subclass of it. |
| `name(value: String)` | has this name. |
| `returns(type: String)` | returns this type. |
| `params(vararg types: String)` | takes exactly these parameters. |
| `hasParam(type: String)` | takes a parameter of this type. |
| `paramCount(count: Int)` | takes this many parameters. |

### `FieldRefMatch`

Describes a field that is accessed.

| Member | Matches fields that |
|---|---|
| `owner(type: String)` | are declared on this class. |
| `name(value: String)` | have this name. |
| `type(descriptor: String)` | have this type. |

### `ArrayElementMatch`

Describes the array an element is read from or written to.

| Member | Matches arrays that |
|---|---|
| `type(descriptor: String)` | hold elements of this type. An object array's type comes from where the array was written, such as a field, a parameter, or a call's result, so an array of unknown type never matches. |

## Points

### Point functions

| Function | Description |
|---|---|
| `MethodTarget.point(debugName: String? = null, block: PointMatch.() -> Unit): PointTarget` | The first instruction in the method that matches. A sequence built with `then` points at its last step. |
| `MethodTarget.points(debugName: String? = null, block: PointMatch.() -> Unit): PointsTarget` | Every matching instruction in the method. |
| `MethodsTarget.points(debugName: String? = null, block: PointMatch.() -> Unit): PointsTarget` | Every matching instruction across these methods. |

### `PointTarget`

One instruction. A point keeps naming the same instruction when code is added around it, anywhere in the method.

| Member | Description |
|---|---|
| `method: MethodTarget` | The method the point is in. |
| `index: Int` | The instruction's position now. It moves as code is inserted before it. |
| `instruction: Instruction` | The instruction. |
| `previous(block: PointMatch.() -> Unit): PointTarget` | The nearest earlier instruction matching the block. It takes a single step, not a sequence. |
| `next(block: PointMatch.() -> Unit): PointTarget` | The nearest later instruction matching the block. On a call, a leading `resultOf` must match the call's own result. |
| `captureAs(name: String, type: String? = null): PointTarget` | Saves the value in this instruction's first register, for `capture(name)` in code added at a later point: the value it writes, or the value a store, a branch or a return takes there. Its type comes from the instruction, or from what wrote the register. |
| `captureArgumentAs(name: String, argument: Int, type: String? = null): PointTarget` | Saves argument `argument` of this instruction, counted as the table below shows. A `long` or `double` argument counts once. |
| `writer(argument: Int, debugName: String? = null): PointTarget` | The instruction that produced argument `argument` of this instruction, counted as the table below shows, following register copies back to the source. Fails when the value is an incoming parameter or comes from several places. |
| `jumpSource(debugName: String? = null): PointTarget` | The branch or switch that jumps to this instruction. An instruction that only falls into it doesn't count. Fails when none or several jump here. |
| `callee(debugName: String? = null): MethodTarget` | The method this call invokes, as a target. It resolves to the nearest declaration up the class hierarchy. |
| `field(debugName: String? = null): FieldTarget` | The field this instruction reads or writes, as a target. |
| `redirectTo(target: ExtMethod)` | Replaces this call or array access with a call to a static extension method. For an instance call, the object comes first. An array read calls `target(array, index)` and the app takes its result as the element; a write calls `target(array, index, value)`. Arguments, the result, and access are checked. |

`argument`, `captureArgumentAs` and `writer` count the values an instruction takes as a call counts its arguments:

| Instruction | Counts as |
|---|---|
| A call | Its arguments, the object first for an instance call. |
| An array read, `aget v, array, index` | `get(array, index)` |
| An array write, `aput value, array, index` | `set(array, index, value)` |
| An instance field read, `iget v, object` | `get(object)` |
| An instance field write, `iput value, object` | `set(object, value)` |
| A static field write, `sput value` | `set(value)` |
| A branch, `if-eqz v` or `if-eq a, b` | `test(v)` or `test(a, b)` |
| A return, `return v` | `return(v)` |
| Arithmetic and conversions, `add-int d, a, b`, `or-int/2addr a, b`, `and-int/lit16 d, a, 0x1000`, `int-to-long d, a` | `op(a, b)`, `op(a, b)`, `op(a)`, `op(a)` |
| A numeric comparison, `cmpl-float d, a, b` | `op(a, b)` |

So the value an `iput` stores is argument 1, and the value an `aput` stores is argument 2. Assigning argument 0 of a branch in `before { }` changes which way the branch goes. Assigning argument 0 of a return changes what that one return gives back.

An array access also counts as a call for `redirectTo`.

Code blocks attach here too: `before`, `after`, and `skipWhen`. See [API: Code blocks](reference_code.md#hooks). Replacing the method's body ends every point in it, and using one afterwards fails.

### `PointsTarget`

Several points, found together before any change, so changes at one don't move the others.

| Member | Description |
|---|---|
| `all: List<PointTarget>` | Every match. |
| `forEach(block: PointTarget.() -> Unit)` | Runs the block with each match as its receiver. |
| `single(): PointTarget` | The one match. Fails, listing every match, unless there is exactly one. |

### `PointMatch`

The receiver of `point { }`, `points { }`, `previous { }`, and `next { }`. Every constraint must hold.

| Member | Matches |
|---|---|
| `opcode(vararg opcodes: Opcode)` | an instruction with one of these opcodes. |
| `string(value: String)` | loading exactly this string. |
| `stringContains(part: String)` | loading a string containing `part`. |
| `literal(value: Long)` | loading this number. |
| `type(descriptor: String)` | an instruction naming this type, such as `const-class` or `instance-of`. |
| `checkCast(type: String)` | a cast to this type. |
| `newInstance(type: String)` | creating an object of this type. |
| `invoke(vararg opcodes: Opcode, block: MethodRefMatch.() -> Unit = {})` | a call with one of these opcodes, to a method matching the block. |
| `invokeStatic(block)`, `invokeVirtual(block)`, `invokeInterface(block)`, `invokeDirect(block)` | a call of that kind. The block is optional. |
| `calls(target: MethodTarget)` | a call to exactly this method. |
| `field(block: FieldRefMatch.() -> Unit)` | reading or writing a matching field. |
| `arrayRead(block: ArrayElementMatch.() -> Unit = {})` | reading an element of a matching array. |
| `arrayWrite(block: ArrayElementMatch.() -> Unit = {})` | writing an element of a matching array. |
| `argument(index: Int, block: PointMatch.() -> Unit)` | an instruction whose argument `index`, counted as for [`PointTarget`](#pointtarget), comes from an instruction matching the block on every path that reaches it. |
| `resultOf(returns: String? = null)` | reading the result of a call (`move-result`) that returns this type, or any type. |
| `where(predicate: Instruction.() -> Boolean)` | your own check on the instruction. |
| `then(within: Int = 1, block: PointMatch.() -> Unit)` | the next step of a sequence, at most `within` instructions later. |

### `ResolvedPoint`

What a point resolves to. You rarely need it.

| Member | Description |
|---|---|
| `method: Method` | The method. |
| `index: Int` | The instruction's current position. |

### `Capture`

A value a point saved with `captureAs` or `captureArgumentAs`. Code blocks read it with `capture(name)`, which returns a [`ValueRef`](reference_code.md#valueref).

| Member | Description |
|---|---|
| `name: String` | The name it was saved under. |
| `type: String` | Its type's descriptor. |
| `register: Int` | The register that holds it. It follows the value when the method needs more registers. |

## Bindings

A binding describes a path through obfuscated objects, such as `post.attributes.videos[0].getUrl()`, by structure. Code blocks then read it with null checks along the way. [Reading obfuscated objects](12_bindings.md) walks through an example.

### `bind`

`bind(debugName: String? = null, block: BindingQuery.() -> Unit): BindingTarget` declares a binding.

### `BindingQuery`

The receiver of `bind { }`. Start with one source, then declare named paths.

| Member | Description |
|---|---|
| `fromField(debugName: String? = null, block: FieldLocator.() -> Unit)` | Starts from the type of a field the locator finds. |
| `fromMethod(target: MethodTarget)` | Starts from a method. Paths can then begin with `self()` (its class) or `param(i)`, and use the steps that scan the method's instructions. |
| `fromClass(target: ClassTarget)` | Starts from a class. |
| `raw(block: PathQuery.() -> Unit)` | The path from the value you pass in to the bound object. Without it, the value you pass in is the bound object. |
| `member(name: String, block: PathQuery.() -> Unit)` | A named path from the bound object, read with `BindingTarget.member(name, value)`. |
| `bind(name: String, target: BindingTarget, block: PathQuery.() -> Unit)` | A named path whose result is another binding's object. |
| `sourceType: String` | The bound object's type. Only known once the binding has been resolved, so read it inside path blocks, for example in `rankBy`. |

### `BindingTarget`

| Member | Description |
|---|---|
| `member(name: String, on: ValueRef): ValueRef` | Walks the named path from `on`, in a code block. If any step is null, or a list is empty, the result is null or zero. |
| `of(value: ValueRef): ValueRef` | Walks the `raw` path from `value`. |
| `sourceType: String` | The bound object's type. |
| `sourceField: FieldTarget` | The field `fromField` found. Read it yourself to get the object: `thisObject.field(post.sourceField)`. |

### `FieldLocator`

The receiver of `fromField { }`, and of `field(name) { }` in a path.

| Member | Description |
|---|---|
| `owner(type: String)` | The class to look in. |
| `firstObjectRead()` | The first object field of `owner` that any of its methods reads. |
| `firstObjectReadAnyOwner()` | The same, for a field on any class. |
| `nearestObjectReadBeforeString(value: String)` | The object field read closest before the string `value` is loaded, in `owner`'s methods. |
| `rankBy(label: String, block: RankScope.() -> Int)` | Scores each instance field of `owner` by its type. The highest wins. |
| `requireScoreAtLeast(score: Int)` | Fails when the best score is below this. |

Without a `firstObjectRead…` or `nearestObjectRead…` rule, the locator picks among `owner`'s instance fields: by `rankBy`, or the only one there is.

### `PathQuery`

One path, step by step. Each step works on the value the previous one produced.

| Member | Step |
|---|---|
| `self()` | Starts from the source method's class. First step only, and only with `fromMethod`. |
| `param(index: Int)` | Starts from the source method's parameter. First step only, and only with `fromMethod`. |
| `member(name: String)` | Starts with a path declared earlier in this binding, or `"raw"`. First step only. |
| `field(type: String)` | Reads the one instance field of this type. |
| `field(name: String, block: FieldLocator.() -> Unit)` | Reads the field a locator finds on the current type. |
| `field(target: FieldTarget)` | Reads this field. |
| `instanceField(typeAnyOf: List<String>)` | Reads the first instance field whose type is one of these, tried in order. |
| `objectSlots().firstInstanceOf(type: String)` | Of the current object's `Object`-typed fields, reads the first that holds an instance of `type`. Checked in the app, at run time. |
| `firstFieldRead()` | Reads the first object field the source method reads. Only with `fromMethod`. |
| `nextFieldRead(owner: String? = null)` | Reads the next object field the source method reads after the previous step. |
| `nextInterfaceCall(returning: String? = null, returningObject: Boolean = false)` | Calls the next interface method the source method calls on the previous step's value. `returning` requires that return type, and `returningObject = true` requires any object type. |
| `callVirtual(owner: String, name: String, proto: String)` | Calls this method. |
| `callInterface(owner: String, name: String, proto: String)` | Calls this interface method. |
| `listGetter(name: String, block: MethodRank.() -> Unit)` | Calls the current class's method that takes nothing and returns a `List`, chosen by the ranking. `name` is a label. |
| `first()` | Takes the list's first element. An empty list gives null. |
| `last()` | Takes the list's last element. |
| `cast(type: String)` | Casts to this type. |

### `SlotQuery`

What `objectSlots()` returns. `firstInstanceOf(type: String)` is its only step.

### `MethodRank`

The receiver of `listGetter { }`. `rankBy(label: String, block: MethodRankScope.() -> Int)` scores each candidate getter.

### `ResolvedBinding`

What a binding resolves to. Patch code reads bindings through `BindingTarget` instead.

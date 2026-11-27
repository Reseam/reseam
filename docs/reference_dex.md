---
title: "API: Raw bytecode"
description: Methods, classes, and instructions as data, for edits that targets and code blocks can't express.
---

# API: Raw bytecode

The layer under targets and code blocks: methods and classes as objects, instructions as data, and registers as numbers. The guide page is [Raw bytecode](13_bytecode.md). The handles and helpers are in `app.reseam.patch.dex`, and the data types in `app.reseam.patch.types`.

Reach it from a target (`target.method`, `target.classDef`) or from [`bytecode`](reference_code.md#bytecodescope). Handles are only valid while the patch that got them runs.

## `app.reseam.patch.dex`

### `Method`

A method in the app.

**Reading**

| Member | Description |
|---|---|
| `handle: UInt` | The engine's id for the method. |
| `info: MethodInfo` | Everything below in one record. |
| `classDef: DexClass` | The class that declares it. |
| `descriptor: String`, `name: String`, `owner: String`, `proto: String` | Its identity. |
| `returnType: String`, `parameterTypes: List<String>` | Its signature's types. |
| `isStatic: Boolean` | Whether it is static. |
| `instructions: List<Instruction>` | Its instructions, in order. Read an instruction's operands through the [`Instruction` properties](#instruction-properties). |
| `instructionCount: Int` | How many instructions it has. |
| `registersSize: Int`, `insSize: Int`, `outsSize: Int` | Its frame: every register, the registers its parameters arrive in, and the most any call it makes passes. |
| `dexIndex: Int` | Which of the app's DEX files holds it. |

**Searching**

Each returns an instruction index, or null when nothing matches.

| Member | Description |
|---|---|
| `indexOfFirst(opcode: Opcode, start: Int = 0): Int?` | The first instruction with this opcode, from `start`. |
| `indexOfFirstReversed(opcode: Opcode, start: Int): Int?` | The last one at or before `start`. |
| `indexOfFirstLiteral(literal: Long): Int?` | The first instruction loading this number. |
| `indexOfFirstLiteralReversed(literal: Long): Int?` | The last one. |
| `containsLiteral(literal: Long): Boolean` | Whether the method loads this number. |
| `indexOfFirstString(value: String): Int?` | The first instruction loading this string. |
| `findAllIndices(opcode: Opcode): List<Int>` | Every instruction with this opcode. |
| `indexOfFirstMethodCall(owner: String, name: String, start: Int = 0): Int?` | The first call to `owner.name`. |
| `indexOfFirstFieldAccess(opcode: Opcode, fieldType: String? = null, owner: String? = null, start: Int = 0): Int?` | The first field access with this opcode, optionally of this type or on this class. |
| `indexOfOpcodeSequence(vararg opcodes: Opcode?, start: Int = 0): Int?` | Where these opcodes run back to back. `null` matches any one instruction. |
| `indexOfFirstInstruction(start: Int = 0, predicate: Instruction.() -> Boolean): Int?` | The first instruction your check accepts. |
| `indexOfFirstInstructionReversed(start: Int? = null, predicate: Instruction.() -> Boolean): Int?` | The last one, searching back from `start` or the end. |
| `findFreeRegister(atIndex: Int, exclude: List<Int> = emptyList()): Int` | A register that holds nothing live at `atIndex`, other than those in `exclude`. |

**Changing**

Indexes you saved go stale when you insert or remove instructions before them. [Points](6_points.md) don't have that problem.

| Member | Description |
|---|---|
| `insertInstructions(index: Int, insns: List<Instruction>)` | Inserts instructions before `index`. |
| `replaceInstruction(index: Int, insn: Instruction)` | Replaces one instruction. |
| `removeInstructions(index: Int, count: Int)` | Removes `count` instructions from `index`. |
| `replaceBody(registersSize: Int, outsSize: Int, insns: List<Instruction>)` | Replaces the whole body and frame. Points and locals in the method stop working. |
| `replaceAllStrings(old: String, new: String): Int` | Replaces every load of exactly `old`. Returns how many changed. |
| `replaceAllLiterals(old: Long, new: Long): Int` | Replaces every load of the number `old`. Returns how many changed. |
| `replaceMethodCall(index: Int, target: MethodRef): Boolean` | Makes the call at `index` call `target` instead. |
| `ensureOutsSize(minOutsSize: Int)` | Grows the frame so calls can pass this many registers. |
| `setAccessFlags(flags: Int)` | Sets its [access flags](#accessflags). |
| `clone(newName: String? = null): Method` | A copy of the method in the same class, under `newName`. |
| `remove()` | Removes the method from its class. |
| `addAnnotation(annotation: AnnotationItem)` | Adds an annotation. |

### `DexClass`

A class in the app.

**Reading**

| Member | Description |
|---|---|
| `handle: UInt` | The engine's id for the class. |
| `info: ClassInfo` | The class's summary in one record. |
| `descriptor: String` | Its descriptor. |
| `superclass: String?` | Its superclass's descriptor. |
| `interfaces: List<String>` | The interfaces it declares. |
| `sourceFile: String?` | The source file name the compiler recorded. |
| `isInterface: Boolean` | Whether it is an interface. |
| `methods: List<Method>` | All its methods. |
| `directMethods: List<Method>`, `virtualMethods: List<Method>` | Its static, private, and constructor methods, and the rest. |
| `fields: List<FieldInfo>` | All its fields. |
| `staticFields: List<FieldInfo>`, `instanceFields: List<FieldInfo>` | Its static and instance fields. |
| `superclassChain: List<DexClass>` | Its superclasses, nearest first, across every DEX file, up to where the platform's classes start. |
| `method(name: String, proto: String? = null): Method?` | A method by name, with `proto` to pick an overload. |
| `field(name: String): FieldRef?` | A field by name. |

**Changing**

| Member | Description |
|---|---|
| `setAccessFlags(flags: Int)` | Sets its access flags. |
| `definal()` | Removes `final`, so other classes can extend it. |
| `setSuperclass(superclass: String)` | Changes its superclass. |
| `addInterface(descriptor: String)` | Makes it implement an interface. A class that already implements it is left as it is. |
| `addMethod(method: NewMethod): Method` | Adds a method from a [`NewMethod`](#newmethod). For most uses, [`ClassTarget.addMethod`](reference_targets.md#method-target-functions) with a code block is simpler. |
| `addField(field: NewField)` | Adds a field. |
| `removeField(name: String)` | Removes a field. |
| `setFieldAccessFlags(fieldName: String, flags: Int)` | Sets a field's access flags. |
| `setStaticFieldValue(fieldName: String, value: EncodedVal)` | Sets a static field's initial value. |
| `addAnnotation(annotation: AnnotationItem)` | Adds an annotation to the class. |
| `addFieldAnnotation(fieldName: String, annotation: AnnotationItem)` | Adds an annotation to a field. |
| `remove()` | Removes the class. |

### Instruction properties

Extension properties that read an [`Instruction`](#instruction) without unpacking its variant. Each is null when the instruction has no such operand.

| Property | Description |
|---|---|
| `opcode: Opcode?` | The opcode. |
| `regA: Int?`, `regB: Int?`, `regC: Int?` | The first, second, and third register operands. Each is null when the instruction's format has no such register: `regB` of `const` or `sget` is null. |
| `referencedRegisters: List<Int>` | Every register the instruction names. |
| `invokeRegisters: List<Int>?` | The registers a call passes. |
| `invokeArgumentTypes: List<String>?` | The types a call passes, the object first for an instance call. |
| `methodRef: MethodRef?` | The method a call names. |
| `fieldRef: FieldRef?` | The field an access names. |
| `stringValue: String?` | The string a `const-string` loads. |
| `typeRef: String?` | The type an instruction names, such as in `new-instance` or `check-cast`. |
| `literal: Long?` | The number an instruction loads. |

### Descriptor helpers

| Member | Description |
|---|---|
| `MethodRef.returnType`, `MethodRef.parameterTypes`, `MethodRef.descriptor` | A reference's return type, parameter types, and full descriptor. |
| `MethodInfo.returnType`, `MethodInfo.parameterTypes`, `MethodInfo.descriptor`, `MethodInfo.isStatic` | The same for a method's info. |
| `FieldInfo.ref: FieldRef` | A field's reference. |
| `parseParameterTypes(proto: String): List<String>` | The parameter types of a prototype. A malformed one fails. |
| `registerWordCount(type: String): Int` | How many registers a value of this type takes: 2 for `long` and `double`, 0 for `void`, 1 otherwise. |
| `isReferenceType(type: String): Boolean` | Whether the type is an object or array type. |

### `Opcode`

Every Dalvik opcode, such as `Opcode.INVOKE_VIRTUAL` or `Opcode.IF_EQZ`.

| Member | Description |
|---|---|
| `value: Int` | The opcode's byte. |
| `isInvoke: Boolean` | Whether it is a call. |
| `isReturn: Boolean` | Whether it is a return. |
| `isMoveResult: Boolean` | Whether it reads a call's result. |
| `endsFlow: Boolean` | Whether execution never falls through to the next instruction: a return, `throw`, or `goto`. |
| `Opcode.of(value: Int): Opcode?` | The opcode with this byte. |

### `AccessFlags`

Flag constants: `PUBLIC`, `PRIVATE`, `PROTECTED`, `STATIC`, `FINAL`, `SYNCHRONIZED`, `VOLATILE`, `BRIDGE`, `TRANSIENT`, `VARARGS`, `NATIVE`, `INTERFACE`, `ABSTRACT`, `STRICT`, `SYNTHETIC`, `ANNOTATION`, `ENUM`, `CONSTRUCTOR`, and `DECLARED_SYNCHRONIZED`. Combine them with `or`.

| Function | Description |
|---|---|
| `Int.isSet(flags: Int)`, `Int.isSet(flags: UInt): Boolean` | Whether `flags` has any bit of this mask. Use it with one constant: `AccessFlags.STATIC.isSet(info.accessFlags)`. |
| `Int.allSet(flags: Int)`, `Int.allSet(flags: UInt): Boolean` | Whether `flags` has every bit of this mask, so `(PUBLIC or FINAL).allSet(flags)` needs both. |

### `InstructionBuilder`

The receiver of `buildInstructions { }`, which returns the instructions as a `List<Instruction>`. You choose every register. Where Dalvik has several encodings of one instruction, the builder picks one: constants and moves by register width, field and array accesses by the value's type, and moves and results by the value's type.

| Member | Instruction |
|---|---|
| `nop()` | `nop` |
| `constInt(dest: Int, value: Int)`, `constLong(dest: Int, value: Long)` | a number constant |
| `constString(dest: Int, value: String)` | `const-string` |
| `constClass(dest: Int, descriptor: String)` | `const-class` |
| `move(dest: Int, src: Int, type: String)` | a move, for a value of `type` |
| `moveResult(dest: Int, type: String)` | reads the last call's result, of `type` |
| `moveException(dest: Int)` | `move-exception` |
| `invokeVirtual`, `invokeSuper`, `invokeDirect`, `invokeStatic`, `invokeInterface` `(owner: String, name: String, proto: String, vararg registers: Int)` | a call |
| `invoke(opcode: Opcode, method: MethodRef, registers: List<Int>)` | a call with any invoke opcode |
| `invokeRange(opcode: Opcode, method: MethodRef, startReg: Int, count: Int)` | a call passing `count` consecutive registers |
| `iget(dest: Int, obj: Int, field: FieldRef)`, `iput(src: Int, obj: Int, field: FieldRef)` | read or write an instance field |
| `sget(dest: Int, field: FieldRef)`, `sput(src: Int, field: FieldRef)` | read or write a static field |
| `newInstance(dest: Int, descriptor: String)` | `new-instance` |
| `newArray(dest: Int, size: Int, descriptor: String)` | `new-array` |
| `arrayLength(dest: Int, array: Int)` | `array-length` |
| `aget(dest: Int, array: Int, index: Int, elementType: String)`, `aput(src: Int, array: Int, index: Int, elementType: String)` | read or write an array element of this type |
| `agetObject(dest: Int, array: Int, index: Int)`, `aputObject(src: Int, array: Int, index: Int)` | read or write an object array element |
| `checkCast(reg: Int, descriptor: String)` | `check-cast` |
| `instanceOf(dest: Int, ref: Int, descriptor: String)` | `instance-of` |
| `goto(offset: Int)` | `goto` |
| `ifEqz`, `ifNez`, `ifLtz`, `ifGez`, `ifGtz`, `ifLez` `(reg: Int, offset: Int)` | a branch comparing with zero |
| `ifEq`, `ifNe`, `ifLt`, `ifGe`, `ifGt`, `ifLe` `(regA: Int, regB: Int, offset: Int)` | a branch comparing two registers |
| `throwValue(reg: Int)` | `throw` |
| `cmplFloat`, `cmpgFloat`, `cmplDouble`, `cmpgDouble`, `cmpLong` `(dest: Int, src1: Int, src2: Int)` | a comparison |
| `addInt`, `subInt`, `mulInt`, `divInt`, `remInt` `(dest: Int, left: Int, right: Int)` | `int` arithmetic |
| `returnVoid()`, `returnValue(reg: Int)`, `returnWide(reg: Int)`, `returnObject(reg: Int)` | a return |
| `reg1(opcode: Opcode, reg: Int)`, `reg2(opcode: Opcode, a: Int, b: Int)`, `reg3(opcode: Opcode, a: Int, b: Int, c: Int)` | any instruction with one to three register operands |
| `literal(opcode: Opcode, dest: Int, value: Long, src: Int = 0)` | any instruction with a number operand |
| `type(opcode: Opcode, a: Int, b: Int, descriptor: String)` | any instruction with a type operand |
| `field(opcode: Opcode, a: Int, b: Int, field: FieldRef)` | any field instruction |
| `add(insn: Instruction)` | an instruction you built yourself |
| `build(): List<Instruction>` | the instructions so far |

Branch offsets are in code units, counted from the branch. The engine widens a branch that ends up out of reach after other changes. A call whose registers neither fit the five-register form nor sit next to each other is refused. Write that call in a code block instead, where the engine chooses the registers.

## `app.reseam.patch.types`

Data types generated from the engine. You read them from instructions, methods, and classes, and build them for raw edits.

### `Instruction`

One instruction. Each variant holds its operands as `field0`, a record named after it: `Reg1` holds a `Reg1Insn`, `Invoke` an `InvokeInsn`, and so on. `Raw` holds the encoded bytes of an instruction the engine passes through unchanged.

The variants: `Simple`, `Reg1`, `Reg2`, `Reg3`, `RegLiteral`, `RegString`, `RegType`, `RegField`, `Invoke`, `InvokeRange`, `Polymorphic`, `PolymorphicRange`, `Custom`, `CustomRange`, `RegHandle`, `RegProto`, `Branch0`, `Branch`, `Branch2`, `FilledArray`, `FilledArrayRange`, `PackedSwitchData`, `SparseSwitchData`, `FillArrayData`, and `Raw`.

Reading operands through the [instruction properties](#instruction-properties) is usually simpler than matching variants.

### Instruction records

Every record has `opcode`, the opcode's byte, except the three data payloads.

| Record | Fields |
|---|---|
| `SimpleInsn` | `opcode` |
| `Reg1Insn` | `opcode`, `regA` |
| `Reg2Insn` | `opcode`, `regA`, `regB` |
| `Reg3Insn` | `opcode`, `regA`, `regB`, `regC` |
| `RegLiteralInsn` | `opcode`, `regA`, `regB`, `literal` |
| `RegStringInsn` | `opcode`, `regA`, `value` |
| `RegTypeInsn` | `opcode`, `regA`, `regB`, `typeDescriptor` |
| `RegFieldInsn` | `opcode`, `regA`, `regB`, `field` |
| `InvokeInsn` | `opcode`, `method`, `registers` |
| `InvokeRangeInsn` | `opcode`, `method`, `startReg`, `regCount` |
| `PolymorphicInsn` | `opcode`, `method`, `proto`, `registers` |
| `PolymorphicRangeInsn` | `opcode`, `method`, `proto`, `startReg`, `regCount` |
| `CustomInsn` | `opcode`, `callSite`, `registers` |
| `CustomRangeInsn` | `opcode`, `callSite`, `startReg`, `regCount` |
| `RegHandleInsn` | `opcode`, `regA`, `handle` |
| `RegProtoInsn` | `opcode`, `regA`, `proto` |
| `Branch0Insn` | `opcode`, `offset` |
| `BranchInsn` | `opcode`, `regA`, `offset` |
| `Branch2Insn` | `opcode`, `regA`, `regB`, `offset` |
| `FilledArrayInsn` | `opcode`, `typeDescriptor`, `registers` |
| `FilledArrayRangeInsn` | `opcode`, `typeDescriptor`, `startReg`, `regCount` |
| `PackedSwitchInsn` | `firstKey`, `targets`: a switch table over consecutive keys |
| `SparseSwitchInsn` | `keys`, `targets`: a switch table over listed keys |
| `FillArrayInsn` | `elementWidth`, `data`: the data of a `fill-array-data` |

`RegLiteralInsn`, `RegTypeInsn` and `RegFieldInsn` also hold formats with one register, such as `const`, `new-instance` and `sget`. Their `regB` is then 0. Read it through `Instruction.regB`, which is null for them.

### `MethodRef`

A reference to a method: `definingClass`, `name`, and `proto`.

### `FieldRef`

A reference to a field: `definingClass`, `name`, and `fieldType`.

### `MethodInfo`

What `Method.info` returns: `classDescriptor`, `methodName`, `proto`, `accessFlags`, `dexIndex`, `registerCount`, `insSize`, `outsSize`, and `instructionCount`.

### `ClassInfo`

What `DexClass.info` returns: `descriptor`, `accessFlags`, `superclass`, `interfaces`, `sourceFile`, `dexIndex`, `directMethodCount`, `virtualMethodCount`, `staticFieldCount`, and `instanceFieldCount`.

### `FieldInfo`

One entry of `DexClass.fields`: `classDescriptor`, `name`, `fieldType`, `accessFlags`, and `initialValue`, the `EncodedVal` a static field starts with.

### `NewMethod`

A method for `DexClass.addMethod`: `name`, `proto`, `accessFlags`, `registersSize`, `insSize`, `outsSize`, `instructions`, `tries: List<TryItem>`, and `catchHandlers: List<CatchHandler>`.

### `NewField`

A field for `DexClass.addField`: `name`, `fieldType`, `accessFlags`, and `initialValue`.

### `TryItem`

A range of a method's code covered by an exception handler: `startAddr` and `insnCount` in code units, and `handlerIdx`, the index of its `CatchHandler`.

### `CatchHandler`

`typedCatches: List<TypedCatch>`, and `catchAllAddr`, where a `catch` of any exception goes, if there is one.

### `TypedCatch`

One `catch`: `exceptionType` and `addr`, where it goes.

### `AnnotationItem`

An annotation for `addAnnotation`: `visibility`, `annotationType`, and `elements`.

### `AnnotationElement`

One element of an annotation: `name` and `value`, an `EncodedVal`.

### `AnnotationValue`

An annotation nested in a value: `annotationType` and `elements`.

### `EncodedVal`

A constant, as DEX files store them. Each variant holds its value as `field0`:

| Variant | Value |
|---|---|
| `Null` | none |
| `BoolVal`, `ByteVal`, `ShortVal`, `CharVal`, `IntVal`, `LongVal`, `FloatVal`, `DoubleVal` | the number or boolean |
| `StringVal` | a `String` |
| `TypeVal` | a type descriptor |
| `ProtoVal` | a method prototype |
| `FieldVal`, `EnumVal` | a `FieldRef` |
| `MethodVal` | a `MethodRef` |
| `HandleVal` | a `HandleRef` |
| `ArrayVal` | a `List<EncodedVal>` |
| `AnnotationVal` | an `AnnotationValue` |

### `HandleRef`

A method handle: `Field`, holding a `FieldHandleRef`, or `Method`, holding a `MethodHandleRef`.

### `FieldHandleRef`

`kind`, the handle's kind, `field`, and `origin`.

### `MethodHandleRef`

`kind`, the handle's kind, `method`, and `origin`.

### `CallSiteRef`

The call site of an `invoke-custom`: `bootstrap`, `name`, `proto`, `arguments`, and `origin`.

### `PoolOrigin`

Where an item came from in the app: `dexIndex`, the DEX file, and `index`, its position in that file's table.

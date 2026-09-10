# Raw bytecode

Targets and code blocks cover the common cases. Underneath them, `app.reseam.patch.dex` exposes methods and classes as handles, instructions as data, and registers as numbers. Use it for a lookup no query expresses or an edit no code block emits. Reach it through `target.method`, `target.classDef`, `bytecode`, or the `methodTarget`, `classTarget`, and `fieldTarget` constructors.

## Method

```kotlin
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode

val method: Method = updateParams.method
method.instructions
method.registersSize
method.indexOfFirst(Opcode.INVOKE_STATIC)
method.indexOfFirstString("basicIntegrity")
method.indexOfFirstInstruction { opcode == Opcode.IGET && fieldRef?.fieldType == Type.Int }
```

Reads: `info`, `owner`, `name`, `proto`, `returnType`, `parameterTypes`, `isStatic`, `instructions`, `instructionCount`, `registersSize`, `insSize`, `outsSize`, `registerA(index)` through `registerD(index)`, `stringRef`, `methodRef`, `fieldRef`, `typeRef`, `wideLiteral`.

Searches return `Int?`: `indexOfFirst`, `indexOfFirstReversed`, `indexOfFirstLiteral`, `indexOfFirstString`, `indexOfFirstMethodCall(owner, name)`, `indexOfFirstFieldAccess`, `indexOfOpcodeSequence`, `indexOfFirstInstruction { }`, `indexOfFirstInstructionReversed { }`, `findAllIndices`.

Mutation: `insertInstruction`, `insertInstructions`, `addInstructions(index) { builder }`, `replaceInstruction`, `removeInstruction`, `setInstructions`, `replaceBody(registersSize, outsSize, instructions)`, `replaceString`, `replaceAllStrings`, `replaceLiteral`, `replaceMethodCall(index, ref)`, `alwaysReturn`, `growLocalRegisters`, `ensureOutsSize`, `findFreeRegister`, `findContiguousFreeRegisters`, `setAccessFlags`, `clone`, `remove`, `addAnnotation`.

Invokes that the 35c format cannot encode are lowered to range invokes on insertion.

## Instructions

`Instruction` is the engine's data type. Extension properties read it: `opcode`, `regA`, `regB`, `regC`, `invokeRegisters`, `methodRef`, `fieldRef`, `stringValue`, `typeRef`, `literal`, `referencedRegisters`, `codeUnitSize`. `Opcode` is an enum with `value`, `isInvoke`, `isReturn`, `isMoveResult`, and `Opcode.of(value)`.

## Instruction builder

```kotlin
method.addInstructions(0) {
    constInt(0, 1)
    returnValue(0)
}
```

Named after the Dalvik instructions:

- `constInt` and `constLong` pick an encoding; `const4`, `const16`, `const32`, `constHigh16`, `constWide16`, `constWide32`, `constWide` pick one explicitly. `constString`, `constClass`.
- `move*`, `moveTyped(dest, src, type)`, `moveResult*`, `moveResultTyped(dest, type)`, `moveException`.
- `invokeStatic(owner, name, proto, registers...)` and the other invoke kinds; `invoke(opcode, ref, registers)`, `invokeRange(opcode, ref, start, count)`.
- `iget*`, `iput*`, `sget*`, `sput*`, with `*Typed` variants that pick by field type.
- `newInstance`, `newArray`, `arrayLength`, `agetObject`, `aputObject`, `checkCast`, `instanceOf`.
- `goto(label)`, `ifEqz` and the other `if*`, `throwValue`, comparisons, arithmetic.
- `returnVoid`, `returnValue`, `returnWide`, `returnObject`.

`label(name)` defines branch targets; offsets are computed when the sequence is built. `buildInstructions { }` builds a list without inserting it. All register arguments are v-numbers.

## Class

`DexClass` has `info`, `descriptor`, `superclass`, `interfaces`, `isInterface`, `methods`, `directMethods`, `virtualMethods`, `fields`, `staticFields`, `instanceFields`, `superclassChain`, `method(name, proto)`, `field(name)`, and mutation: `setAccessFlags`, `setSuperclass`, `addInterface`, `definal`, `remove`, `addMethod(NewMethod)`, `addField(NewField)`, `removeField`, `setFieldAccessFlags`, `setStaticFieldValue`, `addAnnotation`, `addFieldAnnotation`.

`AccessFlags` holds the flag constants; `flag.isSet(flags)` tests them.

Next: [Publishing](10_publish.md).

# Instruction builder

`method.addInstructions(index) { ... }` opens an `InstructionBuilder` block that emits DEX instructions spliced into the method at `index`. The builder resolves branch offsets after the block runs, so forward jumps to labels defined later are fine.

```kotlin
method.addInstructions(0) {
    constString(0, "patched")
    invokeStatic(
        "Landroid/util/Log;", "i",
        "(Ljava/lang/String;Ljava/lang/String;)I",
        0, 0,
    )
    returnVoid()
}
```

All register arguments are v-numbers. The builder does not grow the method's register frame; if you need more registers than the method declares, rewrite the body with `method.replaceBody(...)`.

## Labels and branches

```kotlin
method.addInstructions(idx) {
    ifEqz(0, "skip")
    constString(1, "non-null")
    goto("done")
    label("skip")
    constString(1, "null")
    label("done")
    return_(1)
}
```

Branches: `goto(label)`, `ifEqz`, `ifNez`, `ifLtz`, `ifGez`, `ifGtz`, `ifLez` (one register), `ifEq`, `ifNe`, `ifLt`, `ifGe`, `ifGt`, `ifLe` (two registers). `label(name)` marks the current position. Undefined labels fail at `build()` time.

## Constants

- `const4(dest, value)`: 4-bit signed literal.
- `const16(dest, value)`: 16-bit.
- `const_(dest, value)`: 32-bit.
- `constHigh16(dest, value)`: high 16 bits of a 32-bit value.
- `constWide16`, `constWide32`, `constWide`: 64-bit literals to a wide register pair.
- `constString(dest, value)`: string literal.
- `constClass(dest, descriptor)`: e.g. `"Ljava/lang/String;"`.

## Registers

- `move(dest, src)`, `moveFrom16`, `moveWide`, `moveObject`, `moveObjectFrom16`.
- `moveResult(dest)`, `moveResultWide`, `moveResultObject`: read the return value of the preceding `invoke-*`.
- `moveException(dest)`: first instruction of an exception handler.

## Method calls

```kotlin
invokeStatic("Ljava/lang/Integer;", "parseInt", "(Ljava/lang/String;)I", 0)
moveResult(0)
```

`invokeVirtual`, `invokeSuper`, `invokeDirect`, `invokeStatic`, `invokeInterface`. Each takes `(className, name, proto, vararg registers: Int)`. Register order matches Dalvik's argument order: for non-static calls, the first register is `this`.

Capture return values with `moveResult` (int), `moveResultWide` (long/double), or `moveResultObject` (reference) on the instruction immediately following the invoke.

## Fields

```kotlin
val field = FieldRef("Lcom/example/Api;", "endpoint", "Ljava/lang/String;")
sgetObject(0, field)
```

Instance: `iget`, `igetWide`, `igetObject`, `igetBoolean`, `iput`, `iputWide`, `iputObject`, `iputBoolean`. Static: `sget`, `sgetWide`, `sgetObject`, `sgetBoolean`, `sput`, `sputWide`, `sputObject`, `sputBoolean`. Pass a `FieldRef(definingClass, name, type)`.

## Objects and arrays

- `newInstance(dest, descriptor)`: allocate, then call the constructor with `invokeDirect`.
- `newArray(dest, size, descriptor)`.
- `arrayLength(dest, array)`.
- `agetObject(dest, array, index)`, `aputObject(src, array, index)`.
- `checkCast(reg, descriptor)`.
- `instanceOf(dest, ref, descriptor)`.

## Returns and throws

`returnVoid()`, `return_(reg)`, `returnWide(reg)`, `returnObject(reg)`. `throw_(reg)` raises the exception held in `reg`.

## Raw instructions

`add(insn: Instruction)` appends a pre-built `Instruction`. Use this when the builder lacks a convenience for the opcode you need.

---
description: Read and edit instructions and registers directly, when targets and code blocks aren't enough.
---

# Raw bytecode

Targets and code blocks cover almost every patch. When they don't, the `app.reseam.patch.dex` package gives you methods and classes as objects, instructions as data, and registers as numbers. Reach it from any target, with `target.method` or `target.classDef`, or from `bytecode`.

Prefer code blocks where you can. They choose registers, keep [points](6_points.md) working as code moves, and check the types of what you pass. At this level, you do all of that yourself.

## Reading code

`Method.instructions` is the method's instructions, in order. The [instruction properties](reference_dex.md#instruction-properties) read their operands without unpacking each kind:

```kotlin
import app.reseam.patch.Type
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.opcode

val method = configureDownloads.method
val index = method.indexOfFirstInstruction { opcode == Opcode.IGET && fieldRef?.fieldType == Type.Int }
    ?: error("no int field read")
```

`Method` also has searches by opcode, string, number, call, and field access. Each returns an instruction index, or null.

## Changing code

Build instructions with `buildInstructions { }`, and insert, replace, or remove them by index:

```kotlin
import app.reseam.patch.dex.buildInstructions

method.insertInstructions(
    index,
    buildInstructions {
        constInt(0, 1)
        returnValue(0)
    },
)
```

The builder's functions are named after Dalvik instructions: `constInt`, `constString`, `move`, `moveResult`, `invokeVirtual`, `iget`, `sput`, `ifEqz`, `goto`, `returnObject`, and so on. Where Dalvik has several encodings of one instruction, the builder picks one for you: constants and moves by register width, field and array accesses by the value's type, and moves and results by the value's type.

Here you choose every register. Register `0` above must be free at that point, and `findFreeRegister(index)` finds one that is.

Things to know:

- **Indexes go stale.** Once you insert or remove instructions before an index you saved, it points at the wrong instruction. Search again, or use points, which move with their instruction.
- **Branch offsets are in code units**, counted from the branch. The engine widens a branch that ends up out of reach after other changes.
- **Some calls need a code block.** A call whose registers neither fit the five-register form nor sit next to each other is refused. Write that call in a code block, where the engine arranges the registers.
- **`replaceBody` ends every point** in the method, as `replace { }` does.

## Classes

`DexClass` lists a class's methods and fields, and changes them: add or remove methods and fields, change access flags, the superclass, or the interfaces. For adding a method, [`ClassTarget.addMethod`](7_changing_code.md#adding-a-method-to-an-app-class) with a code block is usually simpler than building a `NewMethod` by hand.

[API: Raw bytecode](reference_dex.md) lists every member, and the data types in `app.reseam.patch.types`.

Next: [Hermes JavaScript](14_hermes.md).

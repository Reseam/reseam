---
description: Read and edit instructions and registers directly when targets and code blocks aren't enough.
---

# Raw bytecode

Targets and code blocks cover almost every patch. When they don't, the `app.reseam.patch.dex` package gives you methods and classes as objects, instructions as data, and registers as numbers. You can reach it from any target (`target.method`, `target.classDef`) or from `bytecode`.

```kotlin
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.buildInstructions

val method = configureDownloads.method
val index = method.indexOfFirstInstruction { opcode == Opcode.IGET && fieldRef?.fieldType == Type.Int }
    ?: error("no int field read")

method.insertInstructions(
    index,
    buildInstructions {
        constInt(0, 1)
        returnValue(0)
    },
)
```

- **`Method`** reads and searches instructions, and edits them: insert, replace, remove, and replace the body.
- **`DexClass`** lists methods and fields, and adds, removes, or changes them.
- **Instructions** are records from `app.reseam.patch.types` (`Instruction`, `MethodRef`, `FieldRef`). Properties such as `opcode`, `regA`, `methodRef`, `stringValue`, and `literal` read them.
- **The builder**, `buildInstructions { }`, is named after Dalvik instructions: `constInt`, `constLong`, `constString`, `move`, `moveResult`, `invoke*`, `iget`, `iput`, `sget`, `sput`, `if*`, `goto`, `return*`. Where Dalvik has several encodings, it picks one: constants and moves by register width, field accesses by the field's type, moves and results by the value's type. A branch takes its displacement in code units, and the engine widens a branch it moves out of reach. A call whose arguments neither fit the five-register form nor sit in consecutive registers is rejected: write it in a code block, where the engine chooses the registers.

Here you choose the registers yourself. Indexes you save go stale when code is inserted before them; [points](7_points.md) don't have that problem.

The [reference](reference.md#appreseampatchdex) lists every member.

Next: [Bundle projects](13_bundles.md).

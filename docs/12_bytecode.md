---
description: Read and edit instructions and registers directly when targets and code blocks aren't enough.
---

# Raw bytecode

Targets and code blocks cover almost every patch. When they don't, the `app.reseam.patch.dex` package gives you methods and classes as objects, instructions as data, and registers as numbers. You can reach it from any target (`target.method`, `target.classDef`) or from `bytecode`.

```kotlin
import app.reseam.patch.dex.Opcode

val method = configureDownloads.method
val index = method.indexOfFirstInstruction { opcode == Opcode.IGET && fieldRef?.fieldType == Type.Int }
    ?: error("no int field read")

method.addInstructions(index) {
    constInt(0, 1)
    returnValue(0)
}
```

- **`Method`** reads instructions and registers, searches them, and edits them: insert, replace, remove, replace the body, grow the register count.
- **`DexClass`** lists methods and fields, and adds, removes, or changes them.
- **Instructions** are records from `app.reseam.patch.types` (`Instruction`, `MethodRef`, `FieldRef`). Properties such as `opcode`, `regA`, `methodRef`, `stringValue`, and `literal` read them.
- **The builder** in `addInstructions { }` is named after Dalvik instructions: `constInt`, `constString`, `invoke*`, `iget*`, `if*`, `goto(label)`, `return*`. `label(name)` marks a branch target.

Here you choose the registers yourself. Indexes you save go stale when code is inserted before them; [points](7_points.md) don't have that problem.

The [reference](reference.md#appreseampatchdex) lists every member.

Next: [Bundle projects](13_bundles.md).

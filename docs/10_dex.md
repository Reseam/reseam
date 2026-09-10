# Raw bytecode

Targets and code blocks cover the common cases. Underneath, `app.reseam.patch.dex` exposes methods and classes as handles, instructions as data, and registers as numbers. Reach it through `target.method`, `target.classDef`, `bytecode`, or the custom target constructors.

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

`Method` reads instructions, registers, and references, searches by opcode, string, literal, call, or predicate, and mutates: insert, replace, remove, `replaceBody`, `alwaysReturn`, `growLocalRegisters`. `DexClass` lists methods and fields and can add, remove, or re-flag them. `Instruction` extension properties (`opcode`, `regA`, `methodRef`, `stringValue`, `literal`, ...) read the engine's instruction type.

The builder inside `addInstructions { }` is named after the Dalvik instructions: `constInt`, `constString`, `move*`, `invoke*`, `iget*`/`iput*`/`sget*`/`sput*` with `*Typed` variants, `newInstance`, `checkCast`, `goto(label)`, `if*`, `return*`. `label(name)` marks a branch target; offsets are computed on build. All registers are v-numbers, and invokes the 35c format cannot encode are lowered to range form on insertion. Everything is listed in the [reference](12_reference.md#appreseampatchdex).

Next: [Publishing](11_publish.md).

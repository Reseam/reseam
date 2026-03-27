@file:Suppress("unused")

package dev.stitch.patch

fun Instruction.opcode(): Int = when (this) {
    is Instruction.Simple -> value0.opcode.toInt()
    is Instruction.Reg1 -> value0.opcode.toInt()
    is Instruction.Reg2 -> value0.opcode.toInt()
    is Instruction.Reg3 -> value0.opcode.toInt()
    is Instruction.RegLiteral -> value0.opcode.toInt()
    is Instruction.RegString -> value0.opcode.toInt()
    is Instruction.RegType -> value0.opcode.toInt()
    is Instruction.RegField -> value0.opcode.toInt()
    is Instruction.Invoke -> value0.opcode.toInt()
    is Instruction.InvokeRange -> value0.opcode.toInt()
    is Instruction.Branch0 -> value0.opcode.toInt()
    is Instruction.Branch -> value0.opcode.toInt()
    is Instruction.Branch2 -> value0.opcode.toInt()
    is Instruction.FilledArray -> value0.opcode.toInt()
    is Instruction.FilledArrayRange -> value0.opcode.toInt()
    is Instruction.PackedSwitchData -> -1
    is Instruction.SparseSwitchData -> -1
    is Instruction.FillArrayData -> -1
    is Instruction.Raw -> value0[0].toInt() and 0xFF
}

fun Instruction.regA(): Int = when (this) {
    is Instruction.Reg1 -> value0.regA.toInt()
    is Instruction.Reg2 -> value0.regA.toInt()
    is Instruction.Reg3 -> value0.regA.toInt()
    is Instruction.RegLiteral -> value0.regA.toInt()
    is Instruction.RegString -> value0.regA.toInt()
    is Instruction.RegType -> value0.regA.toInt()
    is Instruction.RegField -> value0.regA.toInt()
    is Instruction.Branch -> value0.regA.toInt()
    is Instruction.Branch2 -> value0.regA.toInt()
    else -> -1
}

fun Instruction.regB(): Int = when (this) {
    is Instruction.Reg2 -> value0.regB.toInt()
    is Instruction.Reg3 -> value0.regB.toInt()
    is Instruction.RegLiteral -> value0.regB.toInt()
    is Instruction.RegType -> value0.regB.toInt()
    is Instruction.RegField -> value0.regB.toInt()
    is Instruction.Branch2 -> value0.regB.toInt()
    else -> -1
}

fun Instruction.regC(): Int = when (this) {
    is Instruction.Reg3 -> value0.regC.toInt()
    else -> -1
}

fun Instruction.invokeRegisters(): ShortArray? = when (this) {
    is Instruction.Invoke -> value0.registers
    else -> null
}

fun Instruction.methodRef(): MethodRef? = when (this) {
    is Instruction.Invoke -> value0.method
    is Instruction.InvokeRange -> value0.method
    else -> null
}

fun Instruction.fieldRef(): FieldRef? = when (this) {
    is Instruction.RegField -> value0.`field`
    else -> null
}

fun Instruction.stringValue(): String? = when (this) {
    is Instruction.RegString -> value0.`value`
    else -> null
}

fun Instruction.stringRef(): String? = stringValue()

fun Instruction.typeRef(): String? = when (this) {
    is Instruction.RegType -> value0.typeDescriptor
    else -> null
}

val FieldRef.type: String
    get() = fieldType

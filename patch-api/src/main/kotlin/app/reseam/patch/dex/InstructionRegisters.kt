// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.types.Instruction

val Instruction.regA: Int?
    get() =
        when (this) {
            is Instruction.Reg1 -> field0.regA.toInt()
            is Instruction.RegHandle -> field0.regA.toInt()
            is Instruction.RegProto -> field0.regA.toInt()
            is Instruction.Reg2 -> field0.regA.toInt()
            is Instruction.Reg3 -> field0.regA.toInt()
            is Instruction.RegLiteral -> field0.regA.toInt()
            is Instruction.RegString -> field0.regA.toInt()
            is Instruction.RegType -> field0.regA.toInt()
            is Instruction.RegField -> field0.regA.toInt()
            is Instruction.Branch -> field0.regA.toInt()
            is Instruction.Branch2 -> field0.regA.toInt()
            else -> null
        }

val Instruction.regB: Int?
    get() =
        when (this) {
            is Instruction.Reg2 -> field0.regB.toInt()
            is Instruction.Reg3 -> field0.regB.toInt()
            is Instruction.RegLiteral -> field0.regB.toInt()
            is Instruction.RegType -> field0.regB.toInt()
            is Instruction.RegField -> field0.regB.toInt()
            is Instruction.Branch2 -> field0.regB.toInt()
            else -> null
        }

val Instruction.regC: Int?
    get() = (this as? Instruction.Reg3)?.field0?.regC?.toInt()

val Instruction.referencedRegisters: List<Int>
    get() =
        when (this) {
            is Instruction.Simple,
            is Instruction.Branch0,
            is Instruction.PackedSwitchData,
            is Instruction.SparseSwitchData,
            is Instruction.FillArrayData,
            is Instruction.Raw -> emptyList()
            is Instruction.Reg1 -> listOf(field0.regA.toInt())
            is Instruction.RegHandle -> listOf(field0.regA.toInt())
            is Instruction.RegProto -> listOf(field0.regA.toInt())
            is Instruction.Reg2 -> listOf(field0.regA.toInt(), field0.regB.toInt())
            is Instruction.Reg3 ->
                listOf(field0.regA.toInt(), field0.regB.toInt(), field0.regC.toInt())
            is Instruction.RegLiteral -> listOf(field0.regA.toInt(), field0.regB.toInt())
            is Instruction.RegString -> listOf(field0.regA.toInt())
            is Instruction.RegType -> listOf(field0.regA.toInt(), field0.regB.toInt())
            is Instruction.RegField -> listOf(field0.regA.toInt(), field0.regB.toInt())
            is Instruction.Invoke -> field0.registers.map { it.toInt() }
            is Instruction.Polymorphic -> field0.registers.map { it.toInt() }
            is Instruction.Custom -> field0.registers.map { it.toInt() }
            is Instruction.PolymorphicRange ->
                (field0.startReg.toInt() until field0.startReg.toInt() + field0.regCount.toInt())
                    .toList()
            is Instruction.CustomRange ->
                (field0.startReg.toInt() until field0.startReg.toInt() + field0.regCount.toInt())
                    .toList()
            is Instruction.InvokeRange -> invokeRegisters.orEmpty()
            is Instruction.Branch -> listOf(field0.regA.toInt())
            is Instruction.Branch2 -> listOf(field0.regA.toInt(), field0.regB.toInt())
            is Instruction.FilledArray -> field0.registers.map { it.toInt() }
            is Instruction.FilledArrayRange ->
                (field0.startReg.toInt() until field0.startReg.toInt() + field0.regCount.toInt())
                    .toList()
        }

val Instruction.invokeRegisters: List<Int>?
    get() =
        when (this) {
            is Instruction.Invoke -> field0.registers.map { it.toInt() }
            is Instruction.Polymorphic -> field0.registers.map { it.toInt() }
            is Instruction.Custom -> field0.registers.map { it.toInt() }
            is Instruction.PolymorphicRange ->
                (field0.startReg.toInt() until field0.startReg.toInt() + field0.regCount.toInt())
                    .toList()
            is Instruction.CustomRange ->
                (field0.startReg.toInt() until field0.startReg.toInt() + field0.regCount.toInt())
                    .toList()
            is Instruction.InvokeRange ->
                (field0.startReg.toInt() until field0.startReg.toInt() + field0.regCount.toInt())
                    .toList()
            else -> null
        }

/** What an invoke passes, the receiver first for an instance invoke; null for anything else. */
val Instruction.invokeArgumentTypes: List<String>?
    get() =
        when (this) {
            is Instruction.Polymorphic ->
                listOf(field0.method.definingClass) + parseParameterTypes(field0.proto)
            is Instruction.PolymorphicRange ->
                listOf(field0.method.definingClass) + parseParameterTypes(field0.proto)
            is Instruction.Custom -> parseParameterTypes(field0.callSite.proto)
            is Instruction.CustomRange -> parseParameterTypes(field0.callSite.proto)
            else ->
                methodRef?.let { ref ->
                    buildList {
                        if (opcode != Opcode.INVOKE_STATIC && opcode != Opcode.INVOKE_STATIC_RANGE)
                            add(ref.definingClass)
                        addAll(ref.parameterTypes)
                    }
                }
        }

internal data class InvokeArgument(val register: Int, val type: String)

internal fun Instruction.arguments(label: String): List<InvokeArgument> {
    val types =
        requireNotNull(invokeArgumentTypes) {
            "$label: instruction is not an invoke, so it passes no arguments"
        }
    val registers = invokeRegisters.orEmpty()
    require(types.sumOf(::registerWordCount) == registers.size) {
        "$label: malformed invoke argument word count"
    }
    var word = 0
    return types.map { type ->
        val register = registers[word]
        val width = registerWordCount(type)
        require(width != 2 || registers[word + 1] == register + 1) {
            "$label: nonconsecutive wide argument"
        }
        word += width
        InvokeArgument(register, type)
    }
}

// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.dex

import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodInfo
import app.reseam.patch.types.MethodRef

val Instruction.opcodeValue: Int
    get() =
        when (this) {
            is Instruction.Simple -> field0.opcode.toInt()
            is Instruction.Reg1 -> field0.opcode.toInt()
            is Instruction.Reg2 -> field0.opcode.toInt()
            is Instruction.Reg3 -> field0.opcode.toInt()
            is Instruction.RegLiteral -> field0.opcode.toInt()
            is Instruction.RegString -> field0.opcode.toInt()
            is Instruction.RegType -> field0.opcode.toInt()
            is Instruction.RegField -> field0.opcode.toInt()
            is Instruction.Invoke -> field0.opcode.toInt()
            is Instruction.InvokeRange -> field0.opcode.toInt()
            is Instruction.Polymorphic -> field0.opcode.toInt()
            is Instruction.PolymorphicRange -> field0.opcode.toInt()
            is Instruction.Custom -> field0.opcode.toInt()
            is Instruction.CustomRange -> field0.opcode.toInt()
            is Instruction.RegHandle -> field0.opcode.toInt()
            is Instruction.RegProto -> field0.opcode.toInt()
            is Instruction.Branch0 -> field0.opcode.toInt()
            is Instruction.Branch -> field0.opcode.toInt()
            is Instruction.Branch2 -> field0.opcode.toInt()
            is Instruction.FilledArray -> field0.opcode.toInt()
            is Instruction.FilledArrayRange -> field0.opcode.toInt()
            is Instruction.PackedSwitchData,
            is Instruction.SparseSwitchData,
            is Instruction.FillArrayData -> -1
            is Instruction.Raw -> field0.firstOrNull()?.toInt()?.and(0xFF) ?: -1
        }

val Instruction.opcode: Opcode?
    get() = Opcode.of(opcodeValue)

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

val Instruction.methodRef: MethodRef?
    get() =
        when (this) {
            is Instruction.Invoke -> field0.method
            is Instruction.InvokeRange -> field0.method
            is Instruction.Polymorphic -> field0.method
            is Instruction.PolymorphicRange -> field0.method
            else -> null
        }

val Instruction.fieldRef: FieldRef?
    get() = (this as? Instruction.RegField)?.field0?.field

val Instruction.stringValue: String?
    get() = (this as? Instruction.RegString)?.field0?.value

val Instruction.typeRef: String?
    get() =
        when (this) {
            is Instruction.RegType -> field0.typeDescriptor
            is Instruction.FilledArray -> field0.typeDescriptor
            is Instruction.FilledArrayRange -> field0.typeDescriptor
            else -> null
        }

val Instruction.literal: Long?
    get() {
        val bits = (this as? Instruction.RegLiteral)?.field0?.literal ?: return null
        return when (opcode) {
            Opcode.CONST_HIGH16 -> bits.toInt().shl(16).toLong()
            Opcode.CONST_WIDE_HIGH16 -> bits shl 48
            else -> bits
        }
    }

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

val Instruction.codeUnitSize: Int
    get() =
        when (this) {
            is Instruction.PackedSwitchData -> 4 + 2 * field0.targets.size
            is Instruction.SparseSwitchData -> 2 + 4 * field0.keys.size
            is Instruction.FillArrayData -> 4 + (field0.data.size + 1) / 2
            is Instruction.Raw -> {
                require(field0.isNotEmpty() && field0.size % 2 == 0) {
                    "raw instructions require complete code units"
                }
                field0.size / 2
            }
            else -> OpcodeWidths.units(opcodeValue)
        }

val MethodRef.returnType: String
    get() = proto.substring(prototypeReturnStart(proto))

val MethodRef.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

val MethodInfo.returnType: String
    get() = proto.substring(prototypeReturnStart(proto))

val MethodInfo.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

val MethodInfo.isStatic: Boolean
    get() = AccessFlags.STATIC.isSet(accessFlags)

val MethodInfo.descriptor: String
    get() = "$classDescriptor->$methodName$proto"

val MethodRef.descriptor: String
    get() = "$definingClass->$name$proto"

/** The declared parameters of a complete method prototype; malformed descriptors are rejected. */
fun parseParameterTypes(proto: String): List<String> {
    val end = prototypeReturnStart(proto) - 1
    var offset = 1
    return buildList {
        while (offset < end) {
            val next = descriptorEnd(proto, offset)
            add(proto.substring(offset, next))
            offset = next
        }
    }
}

private fun prototypeReturnStart(proto: String): Int {
    require(proto.startsWith('(')) { "Method prototype must start with '(': $proto" }
    var offset = 1
    while (offset < proto.length && proto[offset] != ')') {
        require(proto[offset] != 'V') { "Void is not a parameter type: $proto" }
        offset = descriptorEnd(proto, offset)
    }
    require(offset < proto.length) { "Method prototype has no closing ')': $proto" }
    val returns = offset + 1
    require(descriptorEnd(proto, returns) == proto.length) {
        "Method prototype has trailing data: $proto"
    }
    return returns
}

private fun descriptorEnd(proto: String, start: Int): Int {
    var offset = start
    while (offset < proto.length && proto[offset] == '[') offset++
    require(offset < proto.length) { "Incomplete type at $start in $proto" }
    return when (proto[offset]) {
        'L' -> {
            val end = proto.indexOf(';', offset)
            require(end > offset + 1) { "Incomplete object type at $start in $proto" }
            end + 1
        }
        'Z',
        'B',
        'S',
        'C',
        'I',
        'J',
        'F',
        'D' -> offset + 1
        'V' -> {
            require(offset == start) { "Void cannot be an array element: $proto" }
            offset + 1
        }
        else -> error("Invalid type at $start in $proto")
    }
}

fun registerWordCount(type: String): Int = if (type == "J" || type == "D") 2 else 1

fun isReferenceType(type: String): Boolean = type.startsWith("L") || type.startsWith("[")

fun buildInstructions(block: InstructionBuilder.() -> Unit): List<Instruction> =
    InstructionBuilder().apply(block).build()

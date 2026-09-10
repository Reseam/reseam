// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.dex

import app.reseam.patch.Branch0Insn
import app.reseam.patch.Branch2Insn
import app.reseam.patch.BranchInsn
import app.reseam.patch.FieldRef
import app.reseam.patch.Instruction
import app.reseam.patch.InvokeInsn
import app.reseam.patch.InvokeRangeInsn
import app.reseam.patch.MethodInfo
import app.reseam.patch.MethodRef
import app.reseam.patch.Reg1Insn
import app.reseam.patch.Reg2Insn
import app.reseam.patch.Reg3Insn
import app.reseam.patch.RegFieldInsn
import app.reseam.patch.RegLiteralInsn
import app.reseam.patch.RegStringInsn
import app.reseam.patch.RegTypeInsn
import app.reseam.patch.SimpleInsn

val Instruction.opcodeValue: Int
    get() = when (this) {
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
        is Instruction.PackedSwitchData,
        is Instruction.SparseSwitchData,
        is Instruction.FillArrayData,
        -> -1
        is Instruction.Raw -> value0[0].toInt() and 0xFF
    }

val Instruction.opcode: Opcode?
    get() = Opcode.of(opcodeValue)

val Instruction.regA: Int?
    get() = when (this) {
        is Instruction.Reg1 -> value0.regA.toInt()
        is Instruction.Reg2 -> value0.regA.toInt()
        is Instruction.Reg3 -> value0.regA.toInt()
        is Instruction.RegLiteral -> value0.regA.toInt()
        is Instruction.RegString -> value0.regA.toInt()
        is Instruction.RegType -> value0.regA.toInt()
        is Instruction.RegField -> value0.regA.toInt()
        is Instruction.Branch -> value0.regA.toInt()
        is Instruction.Branch2 -> value0.regA.toInt()
        else -> null
    }

val Instruction.regB: Int?
    get() = when (this) {
        is Instruction.Reg2 -> value0.regB.toInt()
        is Instruction.Reg3 -> value0.regB.toInt()
        is Instruction.RegLiteral -> value0.regB.toInt()
        is Instruction.RegType -> value0.regB.toInt()
        is Instruction.RegField -> value0.regB.toInt()
        is Instruction.Branch2 -> value0.regB.toInt()
        else -> null
    }

val Instruction.regC: Int?
    get() = (this as? Instruction.Reg3)?.value0?.regC?.toInt()

val Instruction.invokeRegisters: List<Int>?
    get() = when (this) {
        is Instruction.Invoke -> value0.registers.map { it.toInt() }
        is Instruction.InvokeRange -> (value0.startReg.toInt() until value0.startReg.toInt() + value0.regCount.toInt()).toList()
        else -> null
    }

val Instruction.methodRef: MethodRef?
    get() = when (this) {
        is Instruction.Invoke -> value0.method
        is Instruction.InvokeRange -> value0.method
        else -> null
    }

val Instruction.fieldRef: FieldRef?
    get() = (this as? Instruction.RegField)?.value0?.field

val Instruction.stringValue: String?
    get() = (this as? Instruction.RegString)?.value0?.value

val Instruction.typeRef: String?
    get() = when (this) {
        is Instruction.RegType -> value0.typeDescriptor
        is Instruction.FilledArray -> value0.typeDescriptor
        is Instruction.FilledArrayRange -> value0.typeDescriptor
        else -> null
    }

val Instruction.literal: Long?
    get() = (this as? Instruction.RegLiteral)?.value0?.literal

val Instruction.referencedRegisters: List<Int>
    get() = when (this) {
        is Instruction.Simple,
        is Instruction.Branch0,
        is Instruction.PackedSwitchData,
        is Instruction.SparseSwitchData,
        is Instruction.FillArrayData,
        is Instruction.Raw -> emptyList()
        is Instruction.Reg1 -> listOf(value0.regA.toInt())
        is Instruction.Reg2 -> listOf(value0.regA.toInt(), value0.regB.toInt())
        is Instruction.Reg3 -> listOf(value0.regA.toInt(), value0.regB.toInt(), value0.regC.toInt())
        is Instruction.RegLiteral -> listOf(value0.regA.toInt(), value0.regB.toInt())
        is Instruction.RegString -> listOf(value0.regA.toInt())
        is Instruction.RegType -> listOf(value0.regA.toInt(), value0.regB.toInt())
        is Instruction.RegField -> listOf(value0.regA.toInt(), value0.regB.toInt())
        is Instruction.Invoke -> value0.registers.map { it.toInt() }
        is Instruction.InvokeRange -> invokeRegisters.orEmpty()
        is Instruction.Branch -> listOf(value0.regA.toInt())
        is Instruction.Branch2 -> listOf(value0.regA.toInt(), value0.regB.toInt())
        is Instruction.FilledArray -> value0.registers.map { it.toInt() }
        is Instruction.FilledArrayRange -> (value0.startReg.toInt() until value0.startReg.toInt() + value0.regCount.toInt()).toList()
    }

val Instruction.codeUnitSize: Int
    get() = when (this) {
        is Instruction.Simple -> 1
        is Instruction.Reg1 -> 1
        is Instruction.Reg2 -> when (opcode) {
            Opcode.MOVE_FROM16, Opcode.MOVE_WIDE_FROM16, Opcode.MOVE_OBJECT_FROM16 -> 2
            Opcode.MOVE_16, Opcode.MOVE_WIDE_16, Opcode.MOVE_OBJECT_16 -> 3
            else -> 1
        }
        is Instruction.Reg3 -> 2
        is Instruction.RegLiteral -> when (opcode) {
            Opcode.CONST_4 -> 1
            Opcode.CONST, Opcode.CONST_WIDE_32 -> 3
            Opcode.CONST_WIDE -> 5
            else -> 2
        }
        is Instruction.RegString -> if (opcode == Opcode.CONST_STRING_JUMBO) 3 else 2
        is Instruction.RegType -> 2
        is Instruction.RegField -> 2
        is Instruction.Invoke -> 3
        is Instruction.InvokeRange -> 3
        is Instruction.Branch0 -> when (opcode) {
            Opcode.GOTO -> 1
            Opcode.GOTO_16 -> 2
            Opcode.GOTO_32 -> 3
            else -> 2
        }
        is Instruction.Branch -> 2
        is Instruction.Branch2 -> 2
        is Instruction.FilledArray -> 3
        is Instruction.FilledArrayRange -> 3
        is Instruction.PackedSwitchData -> 4 + 2 * value0.targets.size
        is Instruction.SparseSwitchData -> 2 + 4 * value0.keys.size
        is Instruction.FillArrayData -> 4 + (value0.data.size + 1) / 2
        is Instruction.Raw -> (value0.size + 1) / 2
    }

val MethodRef.returnType: String
    get() = proto.substringAfterLast(")")

val MethodRef.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

val MethodInfo.returnType: String
    get() = proto.substringAfterLast(")")

val MethodInfo.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

val MethodInfo.isStatic: Boolean
    get() = AccessFlags.STATIC.isSet(accessFlags)

val MethodInfo.descriptor: String
    get() = "$classDescriptor->$methodName$proto"

val MethodRef.descriptor: String
    get() = "$definingClass->$name$proto"

fun parseParameterTypes(proto: String): List<String> {
    val params = proto.substringAfter("(").substringBefore(")")
    val result = mutableListOf<String>()
    var i = 0
    while (i < params.length) {
        val start = i
        while (params[i] == '[') i++
        i = if (params[i] == 'L') params.indexOf(';', i) + 1 else i + 1
        result += params.substring(start, i)
    }
    return result
}

fun registerWordCount(type: String): Int = if (type == "J" || type == "D") 2 else 1

fun isReferenceType(type: String): Boolean = type.startsWith("L") || type.startsWith("[")

class InstructionBuilder {
    private val insns = mutableListOf<Instruction>()
    private val labels = mutableMapOf<String, Int>()
    private val branchFixups = mutableListOf<BranchFixup>()

    private class BranchFixup(
        val index: Int,
        val label: String,
        val rebuild: (Int) -> Instruction,
    )

    fun label(name: String) {
        labels[name] = insns.size
    }

    fun add(insn: Instruction) {
        insns.add(insn)
    }

    fun nop() = add(Instruction.Simple(SimpleInsn(Opcode.NOP.value.toUShort())))
    fun returnVoid() = add(Instruction.Simple(SimpleInsn(Opcode.RETURN_VOID.value.toUShort())))
    fun returnValue(reg: Int) = reg1(Opcode.RETURN, reg)
    fun returnWide(reg: Int) = reg1(Opcode.RETURN_WIDE, reg)
    fun returnObject(reg: Int) = reg1(Opcode.RETURN_OBJECT, reg)

    fun const4(dest: Int, value: Int) = literal(Opcode.CONST_4, dest, value.toLong())
    fun const16(dest: Int, value: Int) = literal(Opcode.CONST_16, dest, value.toLong())
    fun const32(dest: Int, value: Int) = literal(Opcode.CONST, dest, value.toLong())
    fun constHigh16(dest: Int, value: Int) = literal(Opcode.CONST_HIGH16, dest, value.toLong())
    fun constWide16(dest: Int, value: Long) = literal(Opcode.CONST_WIDE_16, dest, value)
    fun constWide32(dest: Int, value: Long) = literal(Opcode.CONST_WIDE_32, dest, value)
    fun constWide(dest: Int, value: Long) = literal(Opcode.CONST_WIDE, dest, value)

    /** The smallest `const` encoding that holds `value`. */
    fun constInt(dest: Int, value: Int) = when {
        dest <= 15 && value in -8..7 -> const4(dest, value)
        value in Short.MIN_VALUE..Short.MAX_VALUE -> const16(dest, value)
        value and 0xFFFF == 0 -> constHigh16(dest, value ushr 16)
        else -> const32(dest, value)
    }

    fun constLong(dest: Int, value: Long) = when {
        value in Short.MIN_VALUE..Short.MAX_VALUE -> constWide16(dest, value)
        value in Int.MIN_VALUE..Int.MAX_VALUE -> constWide32(dest, value)
        else -> constWide(dest, value)
    }

    fun constString(dest: Int, value: String) =
        add(Instruction.RegString(RegStringInsn(Opcode.CONST_STRING.value.toUShort(), dest.toUShort(), value)))

    fun constClass(dest: Int, descriptor: String) = type(Opcode.CONST_CLASS, dest, 0, descriptor)

    fun move(dest: Int, src: Int) = reg2(Opcode.MOVE, dest, src)
    fun moveFrom16(dest: Int, src: Int) = reg2(Opcode.MOVE_FROM16, dest, src)
    fun move16(dest: Int, src: Int) = reg2(Opcode.MOVE_16, dest, src)
    fun moveWide(dest: Int, src: Int) = reg2(Opcode.MOVE_WIDE, dest, src)
    fun moveWideFrom16(dest: Int, src: Int) = reg2(Opcode.MOVE_WIDE_FROM16, dest, src)
    fun moveWide16(dest: Int, src: Int) = reg2(Opcode.MOVE_WIDE_16, dest, src)
    fun moveObject(dest: Int, src: Int) = reg2(Opcode.MOVE_OBJECT, dest, src)
    fun moveObjectFrom16(dest: Int, src: Int) = reg2(Opcode.MOVE_OBJECT_FROM16, dest, src)
    fun moveObject16(dest: Int, src: Int) = reg2(Opcode.MOVE_OBJECT_16, dest, src)
    fun moveResult(dest: Int) = reg1(Opcode.MOVE_RESULT, dest)
    fun moveResultWide(dest: Int) = reg1(Opcode.MOVE_RESULT_WIDE, dest)
    fun moveResultObject(dest: Int) = reg1(Opcode.MOVE_RESULT_OBJECT, dest)
    fun moveException(dest: Int) = reg1(Opcode.MOVE_EXCEPTION, dest)

    /** A move whose encoding fits the registers and whose kind matches `type`. */
    fun moveTyped(dest: Int, src: Int, type: String) {
        val wide = registerWordCount(type) == 2
        val reference = isReferenceType(type)
        when {
            dest <= 15 && src <= 15 -> when {
                wide -> moveWide(dest, src)
                reference -> moveObject(dest, src)
                else -> move(dest, src)
            }
            dest <= 0xFF -> when {
                wide -> moveWideFrom16(dest, src)
                reference -> moveObjectFrom16(dest, src)
                else -> moveFrom16(dest, src)
            }
            else -> when {
                wide -> moveWide16(dest, src)
                reference -> moveObject16(dest, src)
                else -> move16(dest, src)
            }
        }
    }

    fun moveResultTyped(dest: Int, type: String) = when {
        isReferenceType(type) -> moveResultObject(dest)
        registerWordCount(type) == 2 -> moveResultWide(dest)
        else -> moveResult(dest)
    }

    fun invokeVirtual(owner: String, name: String, proto: String, vararg registers: Int) =
        invoke(Opcode.INVOKE_VIRTUAL, MethodRef(owner, name, proto), registers.toList())

    fun invokeSuper(owner: String, name: String, proto: String, vararg registers: Int) =
        invoke(Opcode.INVOKE_SUPER, MethodRef(owner, name, proto), registers.toList())

    fun invokeDirect(owner: String, name: String, proto: String, vararg registers: Int) =
        invoke(Opcode.INVOKE_DIRECT, MethodRef(owner, name, proto), registers.toList())

    fun invokeStatic(owner: String, name: String, proto: String, vararg registers: Int) =
        invoke(Opcode.INVOKE_STATIC, MethodRef(owner, name, proto), registers.toList())

    fun invokeInterface(owner: String, name: String, proto: String, vararg registers: Int) =
        invoke(Opcode.INVOKE_INTERFACE, MethodRef(owner, name, proto), registers.toList())

    fun invoke(opcode: Opcode, method: MethodRef, registers: List<Int>) =
        add(Instruction.Invoke(InvokeInsn(opcode.value.toUShort(), ShortArray(registers.size) { registers[it].toShort() }, method)))

    fun invokeRange(opcode: Opcode, method: MethodRef, startReg: Int, count: Int) =
        add(Instruction.InvokeRange(InvokeRangeInsn(opcode.value.toUShort(), startReg.toUShort(), count.toUShort(), method)))

    fun iget(dest: Int, obj: Int, field: FieldRef) = field(Opcode.IGET, dest, obj, field)
    fun igetWide(dest: Int, obj: Int, field: FieldRef) = field(Opcode.IGET_WIDE, dest, obj, field)
    fun igetObject(dest: Int, obj: Int, field: FieldRef) = field(Opcode.IGET_OBJECT, dest, obj, field)
    fun igetBoolean(dest: Int, obj: Int, field: FieldRef) = field(Opcode.IGET_BOOLEAN, dest, obj, field)
    fun igetByte(dest: Int, obj: Int, field: FieldRef) = field(Opcode.IGET_BYTE, dest, obj, field)
    fun igetChar(dest: Int, obj: Int, field: FieldRef) = field(Opcode.IGET_CHAR, dest, obj, field)
    fun igetShort(dest: Int, obj: Int, field: FieldRef) = field(Opcode.IGET_SHORT, dest, obj, field)
    fun iput(src: Int, obj: Int, field: FieldRef) = field(Opcode.IPUT, src, obj, field)
    fun iputWide(src: Int, obj: Int, field: FieldRef) = field(Opcode.IPUT_WIDE, src, obj, field)
    fun iputObject(src: Int, obj: Int, field: FieldRef) = field(Opcode.IPUT_OBJECT, src, obj, field)
    fun iputBoolean(src: Int, obj: Int, field: FieldRef) = field(Opcode.IPUT_BOOLEAN, src, obj, field)
    fun iputByte(src: Int, obj: Int, field: FieldRef) = field(Opcode.IPUT_BYTE, src, obj, field)
    fun iputChar(src: Int, obj: Int, field: FieldRef) = field(Opcode.IPUT_CHAR, src, obj, field)
    fun iputShort(src: Int, obj: Int, field: FieldRef) = field(Opcode.IPUT_SHORT, src, obj, field)
    fun sget(dest: Int, field: FieldRef) = field(Opcode.SGET, dest, 0, field)
    fun sgetWide(dest: Int, field: FieldRef) = field(Opcode.SGET_WIDE, dest, 0, field)
    fun sgetObject(dest: Int, field: FieldRef) = field(Opcode.SGET_OBJECT, dest, 0, field)
    fun sgetBoolean(dest: Int, field: FieldRef) = field(Opcode.SGET_BOOLEAN, dest, 0, field)
    fun sgetByte(dest: Int, field: FieldRef) = field(Opcode.SGET_BYTE, dest, 0, field)
    fun sgetChar(dest: Int, field: FieldRef) = field(Opcode.SGET_CHAR, dest, 0, field)
    fun sgetShort(dest: Int, field: FieldRef) = field(Opcode.SGET_SHORT, dest, 0, field)
    fun sput(src: Int, field: FieldRef) = field(Opcode.SPUT, src, 0, field)
    fun sputWide(src: Int, field: FieldRef) = field(Opcode.SPUT_WIDE, src, 0, field)
    fun sputObject(src: Int, field: FieldRef) = field(Opcode.SPUT_OBJECT, src, 0, field)
    fun sputBoolean(src: Int, field: FieldRef) = field(Opcode.SPUT_BOOLEAN, src, 0, field)
    fun sputByte(src: Int, field: FieldRef) = field(Opcode.SPUT_BYTE, src, 0, field)
    fun sputChar(src: Int, field: FieldRef) = field(Opcode.SPUT_CHAR, src, 0, field)
    fun sputShort(src: Int, field: FieldRef) = field(Opcode.SPUT_SHORT, src, 0, field)

    /** The `iget` variant for a field of `type`, chosen by the field's type. */
    fun igetTyped(dest: Int, obj: Int, field: FieldRef) = when (field.fieldType) {
        "J", "D" -> igetWide(dest, obj, field)
        "Z" -> igetBoolean(dest, obj, field)
        "B" -> igetByte(dest, obj, field)
        "C" -> igetChar(dest, obj, field)
        "S" -> igetShort(dest, obj, field)
        "I", "F" -> iget(dest, obj, field)
        else -> igetObject(dest, obj, field)
    }

    fun iputTyped(src: Int, obj: Int, field: FieldRef) = when (field.fieldType) {
        "J", "D" -> iputWide(src, obj, field)
        "Z" -> iputBoolean(src, obj, field)
        "B" -> iputByte(src, obj, field)
        "C" -> iputChar(src, obj, field)
        "S" -> iputShort(src, obj, field)
        "I", "F" -> iput(src, obj, field)
        else -> iputObject(src, obj, field)
    }

    fun sgetTyped(dest: Int, field: FieldRef) = when (field.fieldType) {
        "J", "D" -> sgetWide(dest, field)
        "Z" -> sgetBoolean(dest, field)
        "B" -> sgetByte(dest, field)
        "C" -> sgetChar(dest, field)
        "S" -> sgetShort(dest, field)
        "I", "F" -> sget(dest, field)
        else -> sgetObject(dest, field)
    }

    fun sputTyped(src: Int, field: FieldRef) = when (field.fieldType) {
        "J", "D" -> sputWide(src, field)
        "Z" -> sputBoolean(src, field)
        "B" -> sputByte(src, field)
        "C" -> sputChar(src, field)
        "S" -> sputShort(src, field)
        "I", "F" -> sput(src, field)
        else -> sputObject(src, field)
    }

    fun newInstance(dest: Int, descriptor: String) = type(Opcode.NEW_INSTANCE, dest, 0, descriptor)
    fun newArray(dest: Int, size: Int, descriptor: String) = type(Opcode.NEW_ARRAY, dest, size, descriptor)
    fun arrayLength(dest: Int, array: Int) = reg2(Opcode.ARRAY_LENGTH, dest, array)
    fun agetObject(dest: Int, array: Int, index: Int) = reg3(Opcode.AGET_OBJECT, dest, array, index)
    fun aputObject(src: Int, array: Int, index: Int) = reg3(Opcode.APUT_OBJECT, src, array, index)
    fun checkCast(reg: Int, descriptor: String) = type(Opcode.CHECK_CAST, reg, 0, descriptor)
    fun instanceOf(dest: Int, ref: Int, descriptor: String) = type(Opcode.INSTANCE_OF, dest, ref, descriptor)

    fun goto(label: String) {
        val idx = insns.size
        insns.add(Instruction.Branch0(Branch0Insn(Opcode.GOTO.value.toUShort(), 0)))
        branchFixups.add(BranchFixup(idx, label) { offset ->
            val opcode = when (offset) {
                in -128..127 -> Opcode.GOTO
                in -32768..32767 -> Opcode.GOTO_16
                else -> Opcode.GOTO_32
            }
            Instruction.Branch0(Branch0Insn(opcode.value.toUShort(), offset))
        })
    }

    fun ifEqz(reg: Int, label: String) = branch1(Opcode.IF_EQZ, reg, label)
    fun ifNez(reg: Int, label: String) = branch1(Opcode.IF_NEZ, reg, label)
    fun ifLtz(reg: Int, label: String) = branch1(Opcode.IF_LTZ, reg, label)
    fun ifGez(reg: Int, label: String) = branch1(Opcode.IF_GEZ, reg, label)
    fun ifGtz(reg: Int, label: String) = branch1(Opcode.IF_GTZ, reg, label)
    fun ifLez(reg: Int, label: String) = branch1(Opcode.IF_LEZ, reg, label)

    fun ifEq(regA: Int, regB: Int, label: String) = branch2(Opcode.IF_EQ, regA, regB, label)
    fun ifNe(regA: Int, regB: Int, label: String) = branch2(Opcode.IF_NE, regA, regB, label)
    fun ifLt(regA: Int, regB: Int, label: String) = branch2(Opcode.IF_LT, regA, regB, label)
    fun ifGe(regA: Int, regB: Int, label: String) = branch2(Opcode.IF_GE, regA, regB, label)
    fun ifGt(regA: Int, regB: Int, label: String) = branch2(Opcode.IF_GT, regA, regB, label)
    fun ifLe(regA: Int, regB: Int, label: String) = branch2(Opcode.IF_LE, regA, regB, label)

    fun throwValue(reg: Int) = reg1(Opcode.THROW, reg)

    fun cmplFloat(dest: Int, src1: Int, src2: Int) = reg3(Opcode.CMPL_FLOAT, dest, src1, src2)
    fun cmpgFloat(dest: Int, src1: Int, src2: Int) = reg3(Opcode.CMPG_FLOAT, dest, src1, src2)
    fun cmplDouble(dest: Int, src1: Int, src2: Int) = reg3(Opcode.CMPL_DOUBLE, dest, src1, src2)
    fun cmpgDouble(dest: Int, src1: Int, src2: Int) = reg3(Opcode.CMPG_DOUBLE, dest, src1, src2)
    fun cmpLong(dest: Int, src1: Int, src2: Int) = reg3(Opcode.CMP_LONG, dest, src1, src2)

    fun addInt(dest: Int, left: Int, right: Int) = reg3(Opcode.ADD_INT, dest, left, right)
    fun subInt(dest: Int, left: Int, right: Int) = reg3(Opcode.SUB_INT, dest, left, right)
    fun mulInt(dest: Int, left: Int, right: Int) = reg3(Opcode.MUL_INT, dest, left, right)
    fun divInt(dest: Int, left: Int, right: Int) = reg3(Opcode.DIV_INT, dest, left, right)
    fun remInt(dest: Int, left: Int, right: Int) = reg3(Opcode.REM_INT, dest, left, right)
    fun andIntLit16(dest: Int, src: Int, literal: Int) = literal(Opcode.AND_INT_LIT16, dest, literal.toLong(), src)
    fun andIntLit8(dest: Int, src: Int, literal: Int) = literal(Opcode.AND_INT_LIT8, dest, literal.toLong(), src)

    fun reg1(opcode: Opcode, reg: Int) = add(Instruction.Reg1(Reg1Insn(opcode.value.toUShort(), reg.toUShort())))
    fun reg2(opcode: Opcode, a: Int, b: Int) = add(Instruction.Reg2(Reg2Insn(opcode.value.toUShort(), a.toUShort(), b.toUShort())))
    fun reg3(opcode: Opcode, a: Int, b: Int, c: Int) =
        add(Instruction.Reg3(Reg3Insn(opcode.value.toUShort(), a.toUShort(), b.toUShort(), c.toUShort())))
    fun literal(opcode: Opcode, dest: Int, value: Long, src: Int = 0) =
        add(Instruction.RegLiteral(RegLiteralInsn(opcode.value.toUShort(), dest.toUShort(), src.toUShort(), value)))
    fun type(opcode: Opcode, a: Int, b: Int, descriptor: String) =
        add(Instruction.RegType(RegTypeInsn(opcode.value.toUShort(), a.toUShort(), b.toUShort(), descriptor)))
    fun field(opcode: Opcode, a: Int, b: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(opcode.value.toUShort(), a.toUShort(), b.toUShort(), field)))

    fun build(): List<Instruction> {
        val codeUnitOffsets = IntArray(insns.size + 1)
        var cumulative = 0
        for (i in insns.indices) {
            codeUnitOffsets[i] = cumulative
            cumulative += insns[i].codeUnitSize
        }
        codeUnitOffsets[insns.size] = cumulative
        for (fixup in branchFixups) {
            val labelIdx = labels[fixup.label] ?: error("undefined label: '${fixup.label}'")
            insns[fixup.index] = fixup.rebuild(codeUnitOffsets[labelIdx] - codeUnitOffsets[fixup.index])
        }
        return insns.toList()
    }

    private fun branch1(opcode: Opcode, reg: Int, label: String) {
        val idx = insns.size
        insns.add(Instruction.Branch(BranchInsn(opcode.value.toUShort(), reg.toUShort(), 0)))
        branchFixups.add(BranchFixup(idx, label) { offset ->
            Instruction.Branch(BranchInsn(opcode.value.toUShort(), reg.toUShort(), offset))
        })
    }

    private fun branch2(opcode: Opcode, regA: Int, regB: Int, label: String) {
        val idx = insns.size
        insns.add(Instruction.Branch2(Branch2Insn(opcode.value.toUShort(), regA.toUShort(), regB.toUShort(), 0)))
        branchFixups.add(BranchFixup(idx, label) { offset ->
            Instruction.Branch2(Branch2Insn(opcode.value.toUShort(), regA.toUShort(), regB.toUShort(), offset))
        })
    }
}

fun buildInstructions(block: InstructionBuilder.() -> Unit): List<Instruction> =
    InstructionBuilder().apply(block).build()

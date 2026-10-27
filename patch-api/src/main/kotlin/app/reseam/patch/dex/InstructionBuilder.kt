// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.types.Branch0Insn
import app.reseam.patch.types.Branch2Insn
import app.reseam.patch.types.BranchInsn
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.InvokeInsn
import app.reseam.patch.types.InvokeRangeInsn
import app.reseam.patch.types.MethodRef
import app.reseam.patch.types.Reg1Insn
import app.reseam.patch.types.Reg2Insn
import app.reseam.patch.types.Reg3Insn
import app.reseam.patch.types.RegFieldInsn
import app.reseam.patch.types.RegLiteralInsn
import app.reseam.patch.types.RegStringInsn
import app.reseam.patch.types.RegTypeInsn
import app.reseam.patch.types.SimpleInsn

/**
 * Builds instructions named after their Dalvik mnemonics. Where Dalvik has several encodings of one
 * operation, the builder picks the one the operands need: by register width for moves and
 * constants, by type for field accesses, results and moves. A branch takes its displacement in code
 * units; the engine widens one whose target it moves out of reach.
 */
class InstructionBuilder {
    private val insns = mutableListOf<Instruction>()

    fun add(insn: Instruction) {
        insns += insn
    }

    fun nop() = add(Instruction.Simple(SimpleInsn(Opcode.NOP.value.toUShort())))

    fun returnVoid() = add(Instruction.Simple(SimpleInsn(Opcode.RETURN_VOID.value.toUShort())))

    fun returnValue(reg: Int) = reg1(Opcode.RETURN, reg)

    fun returnWide(reg: Int) = reg1(Opcode.RETURN_WIDE, reg)

    fun returnObject(reg: Int) = reg1(Opcode.RETURN_OBJECT, reg)

    fun constInt(dest: Int, value: Int) = literal(constIntOpcode(dest, value), dest, value.toLong())

    fun constLong(dest: Int, value: Long) = literal(constLongOpcode(value), dest, value)

    fun constString(dest: Int, value: String) =
        add(
            Instruction.RegString(
                RegStringInsn(Opcode.CONST_STRING.value.toUShort(), dest.toUShort(), value)
            )
        )

    fun constClass(dest: Int, descriptor: String) = type(Opcode.CONST_CLASS, dest, 0, descriptor)

    /** Copies a value of [type] from [src] to [dest]. */
    fun move(dest: Int, src: Int, type: String) = reg2(moveOpcode(dest, src, type), dest, src)

    /** Stores the result of the preceding invoke, of [type], in [dest]. */
    fun moveResult(dest: Int, type: String) = reg1(moveResultOpcode(type), dest)

    fun moveException(dest: Int) = reg1(Opcode.MOVE_EXCEPTION, dest)

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
        add(
            Instruction.Invoke(
                InvokeInsn(
                    opcode.value.toUShort(),
                    UShortArray(registers.size) { registers[it].toUShort() },
                    method,
                )
            )
        )

    fun invokeRange(opcode: Opcode, method: MethodRef, startReg: Int, count: Int) =
        add(
            Instruction.InvokeRange(
                InvokeRangeInsn(
                    opcode.value.toUShort(),
                    startReg.toUShort(),
                    count.toUShort(),
                    method,
                )
            )
        )

    fun iget(dest: Int, obj: Int, field: FieldRef) =
        field(fieldOpcode(Opcode.IGET, field.fieldType), dest, obj, field)

    fun iput(src: Int, obj: Int, field: FieldRef) =
        field(fieldOpcode(Opcode.IPUT, field.fieldType), src, obj, field)

    fun sget(dest: Int, field: FieldRef) =
        field(fieldOpcode(Opcode.SGET, field.fieldType), dest, 0, field)

    fun sput(src: Int, field: FieldRef) =
        field(fieldOpcode(Opcode.SPUT, field.fieldType), src, 0, field)

    fun newInstance(dest: Int, descriptor: String) = type(Opcode.NEW_INSTANCE, dest, 0, descriptor)

    fun newArray(dest: Int, size: Int, descriptor: String) =
        type(Opcode.NEW_ARRAY, dest, size, descriptor)

    fun arrayLength(dest: Int, array: Int) = reg2(Opcode.ARRAY_LENGTH, dest, array)

    fun agetObject(dest: Int, array: Int, index: Int) = reg3(Opcode.AGET_OBJECT, dest, array, index)

    fun aputObject(src: Int, array: Int, index: Int) = reg3(Opcode.APUT_OBJECT, src, array, index)

    fun checkCast(reg: Int, descriptor: String) = type(Opcode.CHECK_CAST, reg, 0, descriptor)

    fun instanceOf(dest: Int, ref: Int, descriptor: String) =
        type(Opcode.INSTANCE_OF, dest, ref, descriptor)

    fun goto(offset: Int) =
        add(Instruction.Branch0(Branch0Insn(Opcode.GOTO.value.toUShort(), offset)))

    fun ifEqz(reg: Int, offset: Int) = branch1(Opcode.IF_EQZ, reg, offset)

    fun ifNez(reg: Int, offset: Int) = branch1(Opcode.IF_NEZ, reg, offset)

    fun ifLtz(reg: Int, offset: Int) = branch1(Opcode.IF_LTZ, reg, offset)

    fun ifGez(reg: Int, offset: Int) = branch1(Opcode.IF_GEZ, reg, offset)

    fun ifGtz(reg: Int, offset: Int) = branch1(Opcode.IF_GTZ, reg, offset)

    fun ifLez(reg: Int, offset: Int) = branch1(Opcode.IF_LEZ, reg, offset)

    fun ifEq(regA: Int, regB: Int, offset: Int) = branch2(Opcode.IF_EQ, regA, regB, offset)

    fun ifNe(regA: Int, regB: Int, offset: Int) = branch2(Opcode.IF_NE, regA, regB, offset)

    fun ifLt(regA: Int, regB: Int, offset: Int) = branch2(Opcode.IF_LT, regA, regB, offset)

    fun ifGe(regA: Int, regB: Int, offset: Int) = branch2(Opcode.IF_GE, regA, regB, offset)

    fun ifGt(regA: Int, regB: Int, offset: Int) = branch2(Opcode.IF_GT, regA, regB, offset)

    fun ifLe(regA: Int, regB: Int, offset: Int) = branch2(Opcode.IF_LE, regA, regB, offset)

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

    fun reg1(opcode: Opcode, reg: Int) =
        add(Instruction.Reg1(Reg1Insn(opcode.value.toUShort(), reg.toUShort())))

    fun reg2(opcode: Opcode, a: Int, b: Int) =
        add(Instruction.Reg2(Reg2Insn(opcode.value.toUShort(), a.toUShort(), b.toUShort())))

    fun reg3(opcode: Opcode, a: Int, b: Int, c: Int) =
        add(
            Instruction.Reg3(
                Reg3Insn(opcode.value.toUShort(), a.toUShort(), b.toUShort(), c.toUShort())
            )
        )

    fun literal(opcode: Opcode, dest: Int, value: Long, src: Int = 0) =
        add(
            Instruction.RegLiteral(
                RegLiteralInsn(opcode.value.toUShort(), dest.toUShort(), src.toUShort(), value)
            )
        )

    fun type(opcode: Opcode, a: Int, b: Int, descriptor: String) =
        add(
            Instruction.RegType(
                RegTypeInsn(opcode.value.toUShort(), a.toUShort(), b.toUShort(), descriptor)
            )
        )

    fun field(opcode: Opcode, a: Int, b: Int, field: FieldRef) =
        add(
            Instruction.RegField(
                RegFieldInsn(opcode.value.toUShort(), a.toUShort(), b.toUShort(), field)
            )
        )

    fun build(): List<Instruction> = insns.toList()

    private fun branch1(opcode: Opcode, reg: Int, offset: Int) =
        add(Instruction.Branch(BranchInsn(opcode.value.toUShort(), reg.toUShort(), offset)))

    private fun branch2(opcode: Opcode, regA: Int, regB: Int, offset: Int) =
        add(
            Instruction.Branch2(
                Branch2Insn(opcode.value.toUShort(), regA.toUShort(), regB.toUShort(), offset)
            )
        )
}

fun buildInstructions(block: InstructionBuilder.() -> Unit): List<Instruction> =
    InstructionBuilder().apply(block).build()

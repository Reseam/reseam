// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

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

    fun nop() = add(Instruction.Simple(SimpleInsn(Opcodes.NOP.toUShort())))
    fun returnVoid() = add(Instruction.Simple(SimpleInsn(Opcodes.RETURN_VOID.toUShort())))
    fun return_(reg: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.RETURN.toUShort(), reg.toUShort())))
    fun returnWide(reg: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.RETURN_WIDE.toUShort(), reg.toUShort())))
    fun returnObject(reg: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.RETURN_OBJECT.toUShort(), reg.toUShort())))

    fun const4(dest: Int, value: Int) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.CONST_4.toUShort(), dest.toUShort(), 0u, value.toLong())))

    fun const16(dest: Int, value: Int) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.CONST_16.toUShort(), dest.toUShort(), 0u, value.toLong())))

    fun const_(dest: Int, value: Int) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.CONST.toUShort(), dest.toUShort(), 0u, value.toLong())))

    fun constHigh16(dest: Int, value: Int) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.CONST_HIGH16.toUShort(), dest.toUShort(), 0u, value.toLong())))

    fun constWide16(dest: Int, value: Long) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.CONST_WIDE_16.toUShort(), dest.toUShort(), 0u, value)))

    fun constWide32(dest: Int, value: Long) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.CONST_WIDE_32.toUShort(), dest.toUShort(), 0u, value)))

    fun constWide(dest: Int, value: Long) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.CONST_WIDE.toUShort(), dest.toUShort(), 0u, value)))

    fun constString(dest: Int, value: String) =
        add(Instruction.RegString(RegStringInsn(Opcodes.CONST_STRING.toUShort(), dest.toUShort(), value)))

    fun constClass(dest: Int, descriptor: String) =
        add(Instruction.RegType(RegTypeInsn(Opcodes.CONST_CLASS.toUShort(), dest.toUShort(), 0u, descriptor)))

    fun move(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveFrom16(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_FROM16.toUShort(), dest.toUShort(), src.toUShort())))
    fun move16(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_16.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveWide(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_WIDE.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveWideFrom16(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_WIDE_FROM16.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveWide16(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_WIDE_16.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveObject(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_OBJECT.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveObjectFrom16(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_OBJECT_FROM16.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveObject16(dest: Int, src: Int) = add(Instruction.Reg2(Reg2Insn(Opcodes.MOVE_OBJECT_16.toUShort(), dest.toUShort(), src.toUShort())))
    fun moveResult(dest: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.MOVE_RESULT.toUShort(), dest.toUShort())))
    fun moveResultWide(dest: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.MOVE_RESULT_WIDE.toUShort(), dest.toUShort())))
    fun moveResultObject(dest: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.MOVE_RESULT_OBJECT.toUShort(), dest.toUShort())))
    fun moveException(dest: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.MOVE_EXCEPTION.toUShort(), dest.toUShort())))

    fun invokeVirtual(className: String, name: String, proto: String, vararg registers: Int) =
        add(Instruction.Invoke(InvokeInsn(Opcodes.INVOKE_VIRTUAL.toUShort(), ShortArray(registers.size) { registers[it].toShort() }, MethodRef(className, name, proto))))

    fun invokeSuper(className: String, name: String, proto: String, vararg registers: Int) =
        add(Instruction.Invoke(InvokeInsn(Opcodes.INVOKE_SUPER.toUShort(), ShortArray(registers.size) { registers[it].toShort() }, MethodRef(className, name, proto))))

    fun invokeDirect(className: String, name: String, proto: String, vararg registers: Int) =
        add(Instruction.Invoke(InvokeInsn(Opcodes.INVOKE_DIRECT.toUShort(), ShortArray(registers.size) { registers[it].toShort() }, MethodRef(className, name, proto))))

    fun invokeStatic(className: String, name: String, proto: String, vararg registers: Int) =
        add(Instruction.Invoke(InvokeInsn(Opcodes.INVOKE_STATIC.toUShort(), ShortArray(registers.size) { registers[it].toShort() }, MethodRef(className, name, proto))))

    fun invokeInterface(className: String, name: String, proto: String, vararg registers: Int) =
        add(Instruction.Invoke(InvokeInsn(Opcodes.INVOKE_INTERFACE.toUShort(), ShortArray(registers.size) { registers[it].toShort() }, MethodRef(className, name, proto))))

    fun iget(dest: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IGET.toUShort(), dest.toUShort(), obj.toUShort(), field)))

    fun igetWide(dest: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IGET_WIDE.toUShort(), dest.toUShort(), obj.toUShort(), field)))

    fun igetObject(dest: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IGET_OBJECT.toUShort(), dest.toUShort(), obj.toUShort(), field)))

    fun igetBoolean(dest: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IGET_BOOLEAN.toUShort(), dest.toUShort(), obj.toUShort(), field)))

    fun iput(src: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IPUT.toUShort(), src.toUShort(), obj.toUShort(), field)))

    fun iputWide(src: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IPUT_WIDE.toUShort(), src.toUShort(), obj.toUShort(), field)))

    fun iputObject(src: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IPUT_OBJECT.toUShort(), src.toUShort(), obj.toUShort(), field)))

    fun iputBoolean(src: Int, obj: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.IPUT_BOOLEAN.toUShort(), src.toUShort(), obj.toUShort(), field)))

    fun sget(dest: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SGET.toUShort(), dest.toUShort(), 0u, field)))

    fun sgetWide(dest: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SGET_WIDE.toUShort(), dest.toUShort(), 0u, field)))

    fun sgetObject(dest: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SGET_OBJECT.toUShort(), dest.toUShort(), 0u, field)))

    fun sgetBoolean(dest: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SGET_BOOLEAN.toUShort(), dest.toUShort(), 0u, field)))

    fun sput(src: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SPUT.toUShort(), src.toUShort(), 0u, field)))

    fun sputWide(src: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SPUT_WIDE.toUShort(), src.toUShort(), 0u, field)))

    fun sputObject(src: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SPUT_OBJECT.toUShort(), src.toUShort(), 0u, field)))

    fun sputBoolean(src: Int, field: FieldRef) =
        add(Instruction.RegField(RegFieldInsn(Opcodes.SPUT_BOOLEAN.toUShort(), src.toUShort(), 0u, field)))

    fun newInstance(dest: Int, descriptor: String) =
        add(Instruction.RegType(RegTypeInsn(Opcodes.NEW_INSTANCE.toUShort(), dest.toUShort(), 0u, descriptor)))

    fun newArray(dest: Int, size: Int, descriptor: String) =
        add(Instruction.RegType(RegTypeInsn(Opcodes.NEW_ARRAY.toUShort(), dest.toUShort(), size.toUShort(), descriptor)))

    fun arrayLength(dest: Int, array: Int) =
        add(Instruction.Reg2(Reg2Insn(Opcodes.ARRAY_LENGTH.toUShort(), dest.toUShort(), array.toUShort())))

    fun agetObject(dest: Int, array: Int, index: Int) =
        add(Instruction.Reg3(Reg3Insn(Opcodes.AGET_OBJECT.toUShort(), dest.toUShort(), array.toUShort(), index.toUShort())))

    fun aputObject(src: Int, array: Int, index: Int) =
        add(Instruction.Reg3(Reg3Insn(Opcodes.APUT_OBJECT.toUShort(), src.toUShort(), array.toUShort(), index.toUShort())))

    fun checkCast(reg: Int, descriptor: String) =
        add(Instruction.RegType(RegTypeInsn(Opcodes.CHECK_CAST.toUShort(), reg.toUShort(), 0u, descriptor)))

    fun instanceOf(dest: Int, ref: Int, descriptor: String) =
        add(Instruction.RegType(RegTypeInsn(Opcodes.INSTANCE_OF.toUShort(), dest.toUShort(), ref.toUShort(), descriptor)))

    fun goto(label: String) {
        val idx = insns.size
        insns.add(Instruction.Branch0(Branch0Insn(Opcodes.GOTO.toUShort(), 0)))
        branchFixups.add(BranchFixup(idx, label) { offset ->
            when {
                offset in -128..127 -> Instruction.Branch0(Branch0Insn(Opcodes.GOTO.toUShort(), offset))
                offset in -32768..32767 -> Instruction.Branch0(Branch0Insn(Opcodes.GOTO_16.toUShort(), offset))
                else -> Instruction.Branch0(Branch0Insn(Opcodes.GOTO_32.toUShort(), offset))
            }
        })
    }

    fun ifEqz(reg: Int, label: String) = addBranch1(Opcodes.IF_EQZ, reg, label)
    fun ifNez(reg: Int, label: String) = addBranch1(Opcodes.IF_NEZ, reg, label)
    fun ifLtz(reg: Int, label: String) = addBranch1(Opcodes.IF_LTZ, reg, label)
    fun ifGez(reg: Int, label: String) = addBranch1(Opcodes.IF_GEZ, reg, label)
    fun ifGtz(reg: Int, label: String) = addBranch1(Opcodes.IF_GTZ, reg, label)
    fun ifLez(reg: Int, label: String) = addBranch1(Opcodes.IF_LEZ, reg, label)

    fun ifEq(regA: Int, regB: Int, label: String) = addBranch2(Opcodes.IF_EQ, regA, regB, label)
    fun ifNe(regA: Int, regB: Int, label: String) = addBranch2(Opcodes.IF_NE, regA, regB, label)
    fun ifLt(regA: Int, regB: Int, label: String) = addBranch2(Opcodes.IF_LT, regA, regB, label)
    fun ifGe(regA: Int, regB: Int, label: String) = addBranch2(Opcodes.IF_GE, regA, regB, label)
    fun ifGt(regA: Int, regB: Int, label: String) = addBranch2(Opcodes.IF_GT, regA, regB, label)
    fun ifLe(regA: Int, regB: Int, label: String) = addBranch2(Opcodes.IF_LE, regA, regB, label)

    fun throw_(reg: Int) = add(Instruction.Reg1(Reg1Insn(Opcodes.THROW.toUShort(), reg.toUShort())))

    fun cmplFloat(dest: Int, src1: Int, src2: Int) =
        add(Instruction.Reg3(Reg3Insn(Opcodes.CMPL_FLOAT.toUShort(), dest.toUShort(), src1.toUShort(), src2.toUShort())))
    fun cmpgFloat(dest: Int, src1: Int, src2: Int) =
        add(Instruction.Reg3(Reg3Insn(Opcodes.CMPG_FLOAT.toUShort(), dest.toUShort(), src1.toUShort(), src2.toUShort())))
    fun cmplDouble(dest: Int, src1: Int, src2: Int) =
        add(Instruction.Reg3(Reg3Insn(Opcodes.CMPL_DOUBLE.toUShort(), dest.toUShort(), src1.toUShort(), src2.toUShort())))
    fun cmpgDouble(dest: Int, src1: Int, src2: Int) =
        add(Instruction.Reg3(Reg3Insn(Opcodes.CMPG_DOUBLE.toUShort(), dest.toUShort(), src1.toUShort(), src2.toUShort())))
    fun cmpLong(dest: Int, src1: Int, src2: Int) =
        add(Instruction.Reg3(Reg3Insn(Opcodes.CMP_LONG.toUShort(), dest.toUShort(), src1.toUShort(), src2.toUShort())))

    fun andIntLit16(dest: Int, src: Int, literal: Int) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.AND_INT_LIT16.toUShort(), dest.toUShort(), src.toUShort(), literal.toLong())))
    fun andIntLit8(dest: Int, src: Int, literal: Int) =
        add(Instruction.RegLiteral(RegLiteralInsn(Opcodes.AND_INT_LIT8.toUShort(), dest.toUShort(), src.toUShort(), literal.toLong())))

    fun build(): List<Instruction> {
        val codeUnitOffsets = IntArray(insns.size + 1)
        var cumulative = 0
        for (i in insns.indices) {
            codeUnitOffsets[i] = cumulative
            cumulative += insns[i].codeUnitSize()
        }
        codeUnitOffsets[insns.size] = cumulative

        for (fixup in branchFixups) {
            val labelIdx = labels[fixup.label]
                ?: error("undefined label: '${fixup.label}'")
            val offset = codeUnitOffsets[labelIdx] - codeUnitOffsets[fixup.index]
            insns[fixup.index] = fixup.rebuild(offset)
        }

        return insns.toList()
    }

    private fun addBranch1(opcode: Int, reg: Int, label: String) {
        val idx = insns.size
        insns.add(Instruction.Branch(BranchInsn(opcode.toUShort(), reg.toUShort(), 0)))
        branchFixups.add(BranchFixup(idx, label) { offset ->
            Instruction.Branch(BranchInsn(opcode.toUShort(), reg.toUShort(), offset))
        })
    }

    private fun addBranch2(opcode: Int, regA: Int, regB: Int, label: String) {
        val idx = insns.size
        insns.add(Instruction.Branch2(Branch2Insn(opcode.toUShort(), regA.toUShort(), regB.toUShort(), 0)))
        branchFixups.add(BranchFixup(idx, label) { offset ->
            Instruction.Branch2(Branch2Insn(opcode.toUShort(), regA.toUShort(), regB.toUShort(), offset))
        })
    }
}

fun Instruction.codeUnitSize(): Int = when (this) {
    is Instruction.Simple -> 1
    is Instruction.Reg1 -> 1
    is Instruction.Reg2 -> when (value0.opcode.toInt()) {
        Opcodes.MOVE_FROM16, Opcodes.MOVE_WIDE_FROM16, Opcodes.MOVE_OBJECT_FROM16 -> 2
        Opcodes.MOVE_16, Opcodes.MOVE_WIDE_16, Opcodes.MOVE_OBJECT_16 -> 3
        else -> 1
    }
    is Instruction.Reg3 -> 2
    is Instruction.RegLiteral -> when (value0.opcode.toInt()) {
        Opcodes.CONST_4 -> 1
        Opcodes.CONST, Opcodes.CONST_WIDE_32 -> 3
        Opcodes.CONST_WIDE -> 5
        else -> 2
    }
    is Instruction.RegString -> if (value0.opcode.toInt() == Opcodes.CONST_STRING_JUMBO) 3 else 2
    is Instruction.RegType -> 2
    is Instruction.RegField -> 2
    is Instruction.Invoke -> 3
    is Instruction.InvokeRange -> 3
    is Instruction.Branch0 -> when (value0.opcode.toInt()) {
        Opcodes.GOTO -> 1
        Opcodes.GOTO_16 -> 2
        Opcodes.GOTO_32 -> 3
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

fun buildInstructions(block: InstructionBuilder.() -> Unit): List<Instruction> =
    InstructionBuilder().apply(block).build()

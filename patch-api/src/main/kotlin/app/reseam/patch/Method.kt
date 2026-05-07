// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

class Method(val handle: UInt) {
    val info: MethodInfo
        get() = getMethodInfo(handle)
            ?: error("invalid method handle: $handle")

    val classDef: DexClass
        get() = DexClass(findClass(info.classDescriptor)
            ?: error("class not found: ${info.classDescriptor}"))

    val instructions: List<Instruction>
        get() = getInstructions(handle)

    val instructionCount: Int
        get() = instructionCount(handle).toInt()

    val registersSize: Int
        get() = registersSize(handle).toInt()

    val insSize: Int
        get() = insSize(handle).toInt()

    val outsSize: Int
        get() = outsSize(handle).toInt()

    val dexIndex: Int
        get() = methodDex(handle).toInt()

    fun returnEarly() = returnEarly(handle)
    fun returnEarly(value: Int) = returnEarlyInt(handle, value)
    fun returnEarly(value: Boolean) = returnEarlyBool(handle, value)
    fun returnEarly(value: Long) = returnEarlyWide(handle, value)
    fun returnEarlyNull() = returnEarlyObjectNull(handle)

    fun setInstructions(insns: List<Instruction>) =
        setInstructions(handle, lowerInvokeInstructionsForSet(insns))

    fun replaceBody(registersSize: Int, outsSize: Int, insns: List<Instruction>) =
        replaceBody(
            handle,
            registersSize.toUShort(),
            outsSize.toUShort(),
            lowerInvokeInstructionsForSet(insns, registersSize),
        )

    fun insertInstruction(index: Int, insn: Instruction) =
        insertInstructions(index, listOf(insn))

    fun insertInstructions(index: Int, insns: List<Instruction>) =
        insertInstructions(handle, index.toUInt(), lowerInvokeInstructionsAt(index, insns))

    fun addInstructions(index: Int, block: InstructionBuilder.() -> Unit) =
        insertInstructions(index, buildInstructions(block))

    fun replaceInstruction(index: Int, insn: Instruction) {
        val lowered = lowerInvokeInstructionsAt(index, listOf(insn))
        if (lowered.size == 1) {
            replaceInstruction(handle, index.toUInt(), lowered.single())
        } else {
            removeInstruction(handle, index.toUInt())
            insertInstructions(handle, index.toUInt(), lowered)
        }
    }

    fun removeInstruction(index: Int) = removeInstruction(handle, index.toUInt())
    fun removeInstructions(index: Int, count: Int) = removeInstructions(handle, index.toUInt(), count.toUInt())

    fun replaceString(old: String, new: String): Boolean = replaceString(handle, old, new)
    fun replaceAllStrings(old: String, new: String): Int = replaceAllStrings(handle, old, new).toInt()
    fun replaceLiteral(old: Long, new: Long): Boolean = replaceLiteral(handle, old, new)
    fun replaceAllLiterals(old: Long, new: Long): Int = replaceAllLiterals(handle, old, new).toInt()
    fun replaceMethodCall(
        index: Int, newClass: String, newName: String, newProto: String,
    ): Boolean = replaceMethodCall(handle, index.toUInt(), newClass, newName, newProto)

    fun insertInvokeStatic(
        index: Int, className: String, name: String, proto: String, registers: List<Int>,
    ): Boolean = insertInvokeStatic(handle, index.toUInt(), className, name, proto, ShortArray(registers.size) { registers[it].toShort() })

    fun insertInvokeStaticWithMoveResult(
        index: Int, className: String, name: String, proto: String,
        registers: List<Int>, resultRegister: Int, isObject: Boolean = false,
    ): Boolean = insertInvokeStaticWithMoveResult(
        handle, index.toUInt(), className, name, proto,
        ShortArray(registers.size) { registers[it].toShort() }, resultRegister.toUShort(), isObject,
    )

    fun indexOfFirst(op: Int, start: Int = 0): Int? = indexOfFirst(handle, start.toUInt(), op.toUShort())?.toInt()
    fun indexOfFirstReversed(op: Int, start: Int): Int? = indexOfFirstReversed(handle, start.toUInt(), op.toUShort())?.toInt()
    fun indexOfFirstLiteral(literal: Long): Int? = indexOfFirstLiteral(handle, literal)?.toInt()
    fun indexOfFirstLiteralReversed(literal: Long): Int? = indexOfFirstLiteralReversed(handle, literal)?.toInt()
    fun containsLiteral(literal: Long): Boolean = containsLiteral(handle, literal)
    fun indexOfFirstString(s: String): Int? = indexOfFirstString(handle, s)?.toInt()
    fun findAllIndices(op: Int): List<Int> = findAllIndices(handle, op.toUShort()).toList()
    fun indexOfFirstMethodCall(definingClass: String, methodName: String, start: Int = 0): Int? =
        indexOfFirstMethodCall(handle, definingClass, methodName, start.toUInt())?.toInt()
    fun indexOfFirstFieldAccess(opcode: Int, fieldType: String? = null, definingClass: String? = null, start: Int = 0): Int? =
        indexOfFirstFieldAccess(handle, opcode, fieldType, definingClass, start.toUInt())?.toInt()
    fun indexOfOpcodeSequence(opcodes: IntArray, start: Int = 0): Int? =
        indexOfOpcodeSequence(handle, opcodes, start.toUInt())?.toInt()

    fun ensureOutsSize(minOutsSize: Int) = ensureOutsSize(handle, minOutsSize.toUShort())
    fun growLocalRegisters(additionalLocals: Int): Boolean =
        growLocalRegisters(handle, additionalLocals.toUShort())
    fun findFreeRegister(atIndex: Int, exclude: List<Int> = emptyList()): Int =
        findFreeRegister(handle, atIndex.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).toInt()
    fun findFreeRegisters(atIndex: Int, count: Int, exclude: List<Int> = emptyList()): List<Int> =
        findFreeRegisters(handle, atIndex.toUInt(), count.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).map { it.toInt() }
    fun findContiguousFreeRegisters(atIndex: Int, count: Int, exclude: List<Int> = emptyList()): List<Int> =
        findContiguousFreeRegisters(handle, atIndex.toUInt(), count.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).map { it.toInt() }

    fun registerA(index: Int): Int = instructionRegisterA(handle, index.toUInt()).toInt()
    fun registerB(index: Int): Int = instructionRegisterB(handle, index.toUInt()).toInt()
    fun registerC(index: Int): Int = instructionRegisterC(handle, index.toUInt()).toInt()
    fun registerD(index: Int): Int = instructionRegisterD(handle, index.toUInt()).toInt()
    fun wideLiteral(index: Int): Long = instructionWideLiteral(handle, index.toUInt())
    fun stringRef(index: Int): String? = instructionStringRef(handle, index.toUInt())
    fun methodRef(index: Int): MethodRef? = instructionMethodRef(handle, index.toUInt())
    fun fieldRef(index: Int): FieldRef? = instructionFieldRef(handle, index.toUInt())
    fun typeRef(index: Int): String? = instructionTypeRef(handle, index.toUInt())

    fun setAccessFlags(flags: Int) = setMethodAccessFlags(handle, flags.toUInt())
    fun clone(newName: String? = null): Method = Method(cloneMethod(handle, newName))
    fun remove() = removeMethod(handle)
    fun addAnnotation(annotation: AnnotationItem) = addMethodAnnotation(handle, annotation)

    private fun lowerInvokeInstructionsAt(index: Int, insns: List<Instruction>): List<Instruction> {
        val reserved = referencedRegisters(insns)
        return lowerInvokeInstructions(insns) { wordCount ->
            findContiguousFreeRegisters(index, wordCount, reserved.toList())
        }
    }

    private fun lowerInvokeInstructionsForSet(
        insns: List<Instruction>,
        targetRegistersSize: Int = registersSize,
    ): List<Instruction> {
        val reserved = referencedRegisters(insns)
        return lowerInvokeInstructions(insns) { wordCount ->
            findContiguousUnusedRegisters(targetRegistersSize, reserved, wordCount)
                ?: error("no $wordCount consecutive scratch registers available to lower invoke")
        }
    }
}

private enum class InvokeMoveKind {
    NARROW,
    WIDE,
    OBJECT,
}

private data class InvokeArgSpec(
    val wordCount: Int,
    val moveKind: InvokeMoveKind,
)

private fun Method.lowerInvokeInstructions(
    insns: List<Instruction>,
    scratchAllocator: (wordCount: Int) -> List<Int>,
): List<Instruction> {
    val lowered = mutableListOf<Instruction>()
    for (insn in insns) {
        lowered += lowerInvokeInstruction(insn, scratchAllocator)
    }
    return lowered
}

private fun Method.lowerInvokeInstruction(
    insn: Instruction,
    scratchAllocator: (wordCount: Int) -> List<Int>,
): List<Instruction> {
    val invoke = insn as? Instruction.Invoke ?: return listOf(insn)
    val opcode = invoke.value0.opcode.toInt()
    val regs = invoke.value0.registers
    if (invokeIsDirectlyEncodable(regs)) return listOf(insn)

    val rangeOpcode = invokeRangeOpcode(opcode)
        ?: error("opcode 0x${opcode.toString(16)} does not support invoke/range lowering")
    if (invokeRegistersAreConsecutive(regs)) {
        return listOf(
            Instruction.InvokeRange(
                InvokeRangeInsn(
                    rangeOpcode.toUShort(),
                    regs.firstOrNull()?.toUShort() ?: 0.toUShort(),
                    regs.size.toUShort(),
                    invoke.value0.method,
                )
            )
        )
    }

    val specs = invokeArgSpecs(opcode, invoke.value0.method)
    val expectedWords = specs.sumOf { it.wordCount }
    require(expectedWords == regs.size) {
        "invoke ${invoke.value0.method.definingClass}->${invoke.value0.method.name}${invoke.value0.method.proto} " +
            "uses ${regs.size} register words, expected $expectedWords"
    }

    val scratch = scratchAllocator(regs.size)
    require(scratch.size == regs.size) {
        "scratch allocator returned ${scratch.size} registers for ${regs.size}-word invoke"
    }
    require(invokeRegistersAreConsecutive(scratch.map { it.toShort() }.toShortArray())) {
        "scratch allocator returned a non-consecutive register span: $scratch"
    }

    val lowered = mutableListOf<Instruction>()
    var srcIndex = 0
    var destIndex = 0
    for (spec in specs) {
        val src = regs[srcIndex].toInt()
        if (spec.wordCount == 2) {
            require(srcIndex + 1 < regs.size && regs[srcIndex + 1].toInt() == src + 1) {
                "wide invoke arg must occupy consecutive source registers, got ${regs[srcIndex]} and ${regs.getOrNull(srcIndex + 1)}"
            }
        }
        lowered += buildMoveInstruction(spec.moveKind, scratch[destIndex], src)
        srcIndex += spec.wordCount
        destIndex += spec.wordCount
    }

    lowered += Instruction.InvokeRange(
        InvokeRangeInsn(
            rangeOpcode.toUShort(),
            scratch.first().toUShort(),
            regs.size.toUShort(),
            invoke.value0.method,
        )
    )
    return lowered
}

private fun invokeIsDirectlyEncodable(registers: ShortArray): Boolean =
    registers.size <= 5 && registers.all { it.toInt() in 0..15 }

private fun invokeRegistersAreConsecutive(registers: ShortArray): Boolean =
    registers.indices.all { index ->
        index == 0 || registers[index].toInt() == registers[index - 1].toInt() + 1
    }

private fun invokeRangeOpcode(opcode: Int): Int? = when (opcode) {
    Opcodes.INVOKE_VIRTUAL -> Opcodes.INVOKE_VIRTUAL_RANGE
    Opcodes.INVOKE_SUPER -> Opcodes.INVOKE_SUPER_RANGE
    Opcodes.INVOKE_DIRECT -> Opcodes.INVOKE_DIRECT_RANGE
    Opcodes.INVOKE_STATIC -> Opcodes.INVOKE_STATIC_RANGE
    Opcodes.INVOKE_INTERFACE -> Opcodes.INVOKE_INTERFACE_RANGE
    else -> null
}

private fun invokeArgSpecs(opcode: Int, method: MethodRef): List<InvokeArgSpec> {
    val specs = mutableListOf<InvokeArgSpec>()
    if (opcode != Opcodes.INVOKE_STATIC) {
        specs += InvokeArgSpec(1, InvokeMoveKind.OBJECT)
    }
    for (type in method.parameterTypes) {
        specs += when {
            type == "J" || type == "D" -> InvokeArgSpec(2, InvokeMoveKind.WIDE)
            type.startsWith("L") || type.startsWith("[") -> InvokeArgSpec(1, InvokeMoveKind.OBJECT)
            else -> InvokeArgSpec(1, InvokeMoveKind.NARROW)
        }
    }
    return specs
}

private fun buildMoveInstruction(kind: InvokeMoveKind, dest: Int, src: Int): Instruction {
    val opcode = when {
        dest <= 15 && src <= 15 -> when (kind) {
            InvokeMoveKind.NARROW -> Opcodes.MOVE
            InvokeMoveKind.WIDE -> Opcodes.MOVE_WIDE
            InvokeMoveKind.OBJECT -> Opcodes.MOVE_OBJECT
        }
        dest <= 0xFF -> when (kind) {
            InvokeMoveKind.NARROW -> Opcodes.MOVE_FROM16
            InvokeMoveKind.WIDE -> Opcodes.MOVE_WIDE_FROM16
            InvokeMoveKind.OBJECT -> Opcodes.MOVE_OBJECT_FROM16
        }
        else -> when (kind) {
            InvokeMoveKind.NARROW -> Opcodes.MOVE_16
            InvokeMoveKind.WIDE -> Opcodes.MOVE_WIDE_16
            InvokeMoveKind.OBJECT -> Opcodes.MOVE_OBJECT_16
        }
    }
    return Instruction.Reg2(
        Reg2Insn(opcode.toUShort(), dest.toUShort(), src.toUShort())
    )
}

private fun referencedRegisters(insns: List<Instruction>): Set<Int> =
    buildSet {
        for (insn in insns) {
            addAll(insn.referencedRegisters())
        }
    }

private fun Instruction.referencedRegisters(): List<Int> = when (this) {
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
    is Instruction.InvokeRange -> (value0.startReg.toInt() until value0.startReg.toInt() + value0.regCount.toInt()).toList()
    is Instruction.Branch -> listOf(value0.regA.toInt())
    is Instruction.Branch2 -> listOf(value0.regA.toInt(), value0.regB.toInt())
    is Instruction.FilledArray -> value0.registers.map { it.toInt() }
    is Instruction.FilledArrayRange -> (value0.startReg.toInt() until value0.startReg.toInt() + value0.regCount.toInt()).toList()
}

private fun findContiguousUnusedRegisters(
    registerCount: Int,
    excluded: Set<Int>,
    wordCount: Int,
): List<Int>? {
    if (wordCount == 0) return emptyList()
    if (wordCount > registerCount) return null

    for (start in 0..registerCount - wordCount) {
        val candidate = (start until start + wordCount).toList()
        if (candidate.none { it in excluded }) return candidate
    }
    return null
}

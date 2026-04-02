package dev.stitch.patch

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

    fun setInstructions(insns: List<Instruction>) = setInstructions(handle, insns)
    fun insertInstruction(index: Int, insn: Instruction) = insertInstruction(handle, index.toUInt(), insn)
    fun insertInstructions(index: Int, insns: List<Instruction>) = insertInstructions(handle, index.toUInt(), insns)
    fun addInstructions(index: Int, block: InstructionBuilder.() -> Unit) =
        insertInstructions(index, buildInstructions(block))
    fun replaceInstruction(index: Int, insn: Instruction) = replaceInstruction(handle, index.toUInt(), insn)
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

    fun setRegisters(registersSize: Int, outsSize: Int) = setRegisters(handle, registersSize.toUShort(), outsSize.toUShort())
    fun findFreeRegister(atIndex: Int, exclude: List<Int> = emptyList()): Int =
        findFreeRegister(handle, atIndex.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).toInt()
    fun findFreeRegisters(atIndex: Int, count: Int, exclude: List<Int> = emptyList()): List<Int> =
        findFreeRegisters(handle, atIndex.toUInt(), count.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).map { it.toInt() }

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
    fun clonePreserveParameters(): Method = Method(cloneMethodPreserveParameters(handle))
    fun remove() = removeMethod(handle)
    fun addAnnotation(annotation: AnnotationItem) = addMethodAnnotation(handle, annotation)
}

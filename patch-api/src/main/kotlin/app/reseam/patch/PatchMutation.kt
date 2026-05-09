// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.settings.ToggleSetting

private const val ACC_STATIC = 0x08
private const val REPLACE_LOCAL_BUDGET = 16

internal fun Method.compileInsertedCode(
    index: Int,
    captures: List<CodeCapture>,
    block: CodeScope.() -> Unit,
): CompiledCode {
    val emitter = CodeEmitter.forInsertion(this, index, captures)
    emitter.block()
    return emitter.buildInsertedCode()
}

internal fun Method.insertCodeBefore(block: CodeScope.() -> Unit) {
    insertCompiledCode(0, emptyList(), block)
}

fun Method.before(block: CodeScope.() -> Unit) {
    insertCodeBefore(block)
}

internal fun Method.insertCodeBeforeReturns(block: CodeScope.() -> Unit) {
    val returnIndices = instructions.indices
        .filter { index ->
            when (instructions[index].opcode()) {
                Opcodes.RETURN_VOID,
                Opcodes.RETURN,
                Opcodes.RETURN_OBJECT,
                Opcodes.RETURN_WIDE,
                -> true
                else -> false
            }
        }

    for (index in returnIndices.asReversed()) {
        val captures = when (instructions[index].opcode()) {
            Opcodes.RETURN, Opcodes.RETURN_OBJECT, Opcodes.RETURN_WIDE -> {
                listOf(CodeCapture(returnType, registerA(index)))
            }
            else -> emptyList()
        }
        insertCompiledCode(index, captures, block)
    }
}

private fun Method.insertCompiledCode(
    index: Int,
    captures: List<CodeCapture>,
    block: CodeScope.() -> Unit,
) {
    val compiled = compileInsertedCode(index, captures, block)
    if (compiled.localGrowth > 0) {
        require(growLocalRegisters(compiled.localGrowth)) {
            "Cannot insert code in ${info.classDescriptor}->${info.methodName}${info.proto}[$index]: " +
                "failed to grow method locals by ${compiled.localGrowth}"
        }
    }
    insertInstructions(index, compiled.instructions)
}

fun Method.after(block: CodeScope.() -> Unit) {
    insertCodeBeforeReturns(block)
}

internal fun Method.replaceWithCode(block: CodeScope.() -> Unit) {
    val emitter = CodeEmitter.forReplacement(this)
    emitter.block()
    val plan = emitter.buildReplacementPlan()
    replaceBody(plan.registersSize, plan.outsSize, plan.instructions)
}

fun Method.replace(block: CodeScope.() -> Unit) {
    replaceWithCode(block)
}

internal fun Method.constantAfterEnum(enumType: String, enumValue: String): Int {
    var enumIndex = -1
    for (index in instructions.indices) {
        val insn = instructions[index]
        if (insn is Instruction.RegField &&
            insn.opcode() == Opcodes.SGET_OBJECT &&
            insn.value0.field.definingClass == enumType &&
            insn.value0.field.name == enumValue
        ) {
            enumIndex = index
            break
        }
    }
    require(enumIndex >= 0) {
        "Enum constant $enumType.$enumValue not found in ${info.classDescriptor}->${info.methodName}${info.proto}"
    }
    var literalIndex = -1
    for (index in (enumIndex + 1) until instructions.size) {
        if (instructions[index] is Instruction.RegLiteral) {
            literalIndex = index
            break
        }
    }
    require(literalIndex >= 0) {
        "No literal found after $enumType.$enumValue in ${info.classDescriptor}->${info.methodName}${info.proto}"
    }
    return (instructions[literalIndex] as Instruction.RegLiteral).value0.literal.toInt()
}

internal data class CodeCapture(
    val type: String,
    val register: Int,
)

internal data class ReplacementPlan(
    val registersSize: Int,
    val outsSize: Int,
    val instructions: List<Instruction>,
)

internal data class CompiledCode(
    val instructions: List<Instruction>,
    val localGrowth: Int,
)

internal enum class RegisterConstraint(val maxRegister: Int) {
    LOW(15),
    BYTE(0xFF),
    ANY(0xFFFF),
}

private data class TempAllocation(
    val baseRegister: Int,
    val wordCount: Int,
    val constraint: RegisterConstraint,
)

internal class CodeEmitter private constructor(
    internal val method: Method,
    private val insertIndex: Int?,
    private val replaceMode: Boolean,
    private val captures: List<CodeCapture>,
) : CodeScope {
    internal val builder = AllocatingInstructionBuilder()
    private val usedRegisters = mutableSetOf<Int>()
    private var nextReplaceLocal = 0
    private var nextTempId = 0
    private var plannedLocalGrowth = 0
    private var maxOutRegisters = 0
    private var labelCounter = 0
    private val tempAllocations = mutableMapOf<Int, TempAllocation>()

    companion object {
        fun forInsertion(method: Method, index: Int, captures: List<CodeCapture>): CodeEmitter =
            CodeEmitter(method, index, replaceMode = false, captures = captures)

        fun forReplacement(method: Method): CodeEmitter =
            CodeEmitter(method, insertIndex = null, replaceMode = true, captures = emptyList())
    }

    private val isStaticMethod: Boolean = (method.info.accessFlags.toInt() and ACC_STATIC) != 0
    private val originalRegistersSize: Int = if (replaceMode) REPLACE_LOCAL_BUDGET + method.insSize else method.registersSize
    private val originalIncomingBase: Int = if (replaceMode) REPLACE_LOCAL_BUDGET else method.registersSize - method.insSize
    private val incomingBase: Int = originalIncomingBase

    private val thisRegister: Int
        get() = requireIncomingThis()

    private fun requireIncomingThis(): Int {
        require(!isStaticMethod) {
            "thisObject() is not available in static ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
        }
        return incomingBase
    }

    override fun thisObject(): ValueRef =
        ValueRefImpl(this, thisRegister, method.info.classDescriptor)

    override fun parameter(index: Int): ValueRef {
        val params = method.parameterTypes
        require(index in params.indices) {
            "Parameter index $index out of bounds for ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
        }
        val register = incomingBase +
            (if (isStaticMethod) 0 else 1) +
            params.take(index).sumOf(::registerWordCount)
        return ValueRefImpl(this, register, params[index])
    }

    override fun parameterOfType(type: String): ValueRef {
        val index = method.parameterTypes.indexOf(type)
        require(index >= 0) {
            "No parameter of type $type in ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
        }
        return parameter(index)
    }

    override fun parameterLast(): ValueRef =
        parameter(method.parameterTypes.lastIndex)

    override fun capture(name: String, type: String): ValueRef {
        val capture = captures.firstOrNull { it.type == type }
            ?: error("No capture named '$name' of type $type is available in ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}")
        return ValueRefImpl(this, capture.register, type)
    }

    override fun enumObject(enumType: String, enumValue: String): ValueRef {
        val dest = allocTemp()
        builder.sgetObject(dest, FieldRef(enumType, enumValue, enumType))
        return ValueRefImpl(this, dest, enumType)
    }

    override fun int(value: Int): ValueRef {
        val dest = allocTemp()
        builder.constInt(dest, value)
        return ValueRefImpl(this, dest, "I")
    }

    override fun string(value: String): ValueRef {
        val dest = allocTemp()
        builder.constString(dest, value)
        return ValueRefImpl(this, dest, "Ljava/lang/String;")
    }

    override fun nullObject(): ValueRef {
        val dest = allocTemp()
        builder.const16(dest, 0)
        return ValueRefImpl(this, dest, "Ljava/lang/Object;")
    }

    override fun staticCall(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef =
        invoke(owner, name, proto, args.map { it.asImpl() }, invokeKind = InvokeKind.STATIC)

    override fun ifTrue(value: ValueRef, block: CodeScope.() -> Unit) {
        val done = nextLabel("if_true_done")
        builder.ifEqz(value.asImpl().asByteRegister().register, done)
        this.block()
        builder.label(done)
    }

    override fun ifEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit) {
        val done = nextLabel("if_equal_done")
        val leftReg = left.asImpl().asLowRegister().register
        val rightReg = right.asImpl().asLowRegister().register
        builder.ifNe(leftReg, rightReg, done)
        this.block()
        builder.label(done)
    }

    override fun ifNotNull(value: ValueRef, block: CodeScope.() -> Unit) {
        val done = nextLabel("if_not_null_done")
        builder.ifEqz(value.asImpl().asByteRegister().register, done)
        this.block()
        builder.label(done)
    }

    override fun returnVoid() {
        builder.returnVoid()
    }

    override fun returnTrue() {
        val value = int(1)
        builder.return_(value.asImpl().register)
    }

    override fun returnFalse() {
        val value = int(0)
        builder.return_(value.asImpl().register)
    }

    override fun returnObject(value: ValueRef) {
        builder.returnObject(value.asImpl().asByteRegister().register)
    }

    override fun returnInt(value: ValueRef) {
        builder.return_(value.asImpl().asByteRegister().register)
    }

    internal fun buildInsertedCode(): CompiledCode =
        CompiledCode(
            instructions = builder.build(::resolveRegister),
            localGrowth = plannedLocalGrowth,
        )

    internal fun build(): List<Instruction> =
        builder.build(::resolveRegister)

    internal fun buildReplacementPlan(): ReplacementPlan =
        ReplacementPlan(
            registersSize = REPLACE_LOCAL_BUDGET + method.insSize,
            outsSize = maxOutRegisters,
            instructions = build(),
        )

    internal fun valueField(value: ValueRefImpl, type: String): ValueRef {
        val ownerClass = classFor(value.type)
        val fields = ownerClass.instanceFields.filter { it.fieldType == type }
        require(fields.size == 1) {
            "Expected exactly one instance field of type $type on ${value.type}, found ${fields.size}"
        }
        return valueField(value, FieldRef(value.type, fields.single().name, type))
    }

    internal fun valueField(value: ValueRefImpl, field: FieldRef): ValueRef {
        val type = field.fieldType
        val wordCount = registerWordCount(type)
        val dest = allocTemp(wordCount = wordCount, constraint = RegisterConstraint.LOW)
        val obj = value.asLowRegister()
        when (type) {
            "J", "D" -> builder.igetWide(dest, obj.register, field)
            "Z" -> builder.igetBoolean(dest, obj.register, field)
            "B" -> builder.igetByte(dest, obj.register, field)
            "C" -> builder.igetChar(dest, obj.register, field)
            "S" -> builder.igetShort(dest, obj.register, field)
            "I", "F" -> builder.iget(dest, obj.register, field)
            else -> builder.igetObject(dest, obj.register, field)
        }
        return ValueRefImpl(this, dest, type)
    }

    internal fun invoke(owner: String, name: String, proto: String, args: List<ValueRefImpl>, invokeKind: InvokeKind): ValueRef {
        val registers = args.flatMap { it.registerWords() }
        maxOutRegisters = maxOf(maxOutRegisters, registers.size)
        val opcode = when (invokeKind) {
            InvokeKind.STATIC -> Opcodes.INVOKE_STATIC
            InvokeKind.VIRTUAL -> Opcodes.INVOKE_VIRTUAL
            InvokeKind.INTERFACE -> Opcodes.INVOKE_INTERFACE
        }
        emitInvoke(owner, name, proto, opcode, args, registers)

        val returnType = proto.substringAfter(')')
        return if (returnType == "V") {
            ValueRefImpl(this, VOID_REGISTER, "V")
        } else {
            val dest = allocTemp(registerWordCount(returnType))
            when {
                returnType.startsWith("L") || returnType.startsWith("[") -> builder.moveResultObject(dest)
                returnType == "J" || returnType == "D" -> builder.moveResultWide(dest)
                else -> builder.moveResult(dest)
            }
            ValueRefImpl(this, dest, returnType)
        }
    }

    private fun emitInvoke(
        owner: String,
        name: String,
        proto: String,
        opcode: Int,
        args: List<ValueRefImpl>,
        registers: List<Int>,
    ) {
        val resolvedRegisters = registers.map(::resolvedRegisterForConstraintCheck)
        if (invokeIsDirectlyEncodable(resolvedRegisters)) {
            builder.invoke(opcode, owner, name, proto, registers)
            return
        }

        val rangeOpcode = invokeRangeOpcode(opcode)
            ?: error("opcode 0x${opcode.toString(16)} does not support invoke/range lowering")
        if (invokeRegistersAreConsecutive(resolvedRegisters)) {
            builder.invokeRange(rangeOpcode, owner, name, proto, registers.firstOrNull() ?: 0, registers.size)
            return
        }

        val sourceRegisters = resolvedRegisters
            .filter { it >= 0 }
            .toSet()
        val scratchBase = allocTemp(
            wordCount = registers.size,
            constraint = RegisterConstraint.ANY,
            excludeRegisters = sourceRegisters,
        )
        val scratchRegisters = registerWords(scratchBase, registers.size)
        var destIndex = 0
        for (arg in args) {
            moveValue(scratchRegisters[destIndex], arg.register, arg.type)
            destIndex += arg.wordCount
        }

        builder.invokeRange(rangeOpcode, owner, name, proto, scratchBase, registers.size)
    }

    private fun invokeIsDirectlyEncodable(registers: List<Int>): Boolean =
        registers.size <= 5 && registers.all { it in 0..15 }

    private fun invokeRegistersAreConsecutive(registers: List<Int>): Boolean =
        registers.indices.all { index ->
            index == 0 || registers[index] == registers[index - 1] + 1
        }

    private fun invokeRangeOpcode(opcode: Int): Int? = when (opcode) {
        Opcodes.INVOKE_VIRTUAL -> Opcodes.INVOKE_VIRTUAL_RANGE
        Opcodes.INVOKE_STATIC -> Opcodes.INVOKE_STATIC_RANGE
        Opcodes.INVOKE_INTERFACE -> Opcodes.INVOKE_INTERFACE_RANGE
        else -> null
    }

    internal fun listSize(value: ValueRefImpl): ValueRef =
        invoke("Ljava/util/List;", "size", "()I", listOf(value), InvokeKind.INTERFACE)

    internal fun listGet(value: ValueRefImpl, index: ValueRefImpl): ValueRef =
        invoke("Ljava/util/List;", "get", "(I)Ljava/lang/Object;", listOf(value, index), InvokeKind.INTERFACE)

    internal fun subtract(left: ValueRefImpl, right: ValueRefImpl): ValueRef {
        val dest = allocTemp()
        val leftReg = left.asByteRegister().register
        val rightReg = right.asByteRegister().register
        builder.subInt(dest, leftReg, rightReg)
        return ValueRefImpl(this, dest, "I")
    }

    internal fun cast(value: ValueRefImpl, type: String): ValueRef {
        val target = value.asByteRegister()
        builder.checkCast(target.register, type)
        return ValueRefImpl(this, target.register, type)
    }

    internal fun allocTemp(
        wordCount: Int = 1,
        constraint: RegisterConstraint = RegisterConstraint.BYTE,
        excludeRegisters: Set<Int> = emptySet(),
    ): Int {
        require(wordCount > 0) { "wordCount must be positive" }
        val tempId = nextTempId++
        val register = virtualRegister(tempId)
        if (replaceMode) {
            require(nextReplaceLocal + wordCount <= REPLACE_LOCAL_BUDGET) {
                "CodeScope exceeded local budget $REPLACE_LOCAL_BUDGET for ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
            }
            tempAllocations[tempId] = TempAllocation(nextReplaceLocal, wordCount, constraint)
            nextReplaceLocal += wordCount
            return register
        }

        val allocation = allocateInsertionTemp(wordCount, constraint, excludeRegisters)
        tempAllocations[tempId] = TempAllocation(allocation, wordCount, constraint)
        return register
    }

    private fun allocateInsertionTemp(
        wordCount: Int,
        constraint: RegisterConstraint,
        excludeRegisters: Set<Int>,
    ): Int {
        val index = insertIndex ?: 0
        val registers = method.findContiguousFreeRegisters(
            index,
            wordCount,
            (usedRegisters + excludeRegisters).toList(),
        )
        if (registers.size == wordCount && registers.last() <= constraint.maxRegister) {
            usedRegisters += registers
            return registers.first()
        }

        val grownBase = originalIncomingBase + plannedLocalGrowth
        val grownLast = grownBase + wordCount - 1
        val newRegistersSize = originalRegistersSize + plannedLocalGrowth + wordCount
        require(grownLast <= constraint.maxRegister && newRegistersSize <= UShort.MAX_VALUE.toInt()) {
            "CodeScope cannot allocate $wordCount ${constraint.name.lowercase()} scratch register(s) at " +
                "${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}[$index]; " +
                "free candidates=$registers, plannedLocalGrowth=$plannedLocalGrowth, " +
                "registersSize=$originalRegistersSize, insSize=${method.insSize}"
        }

        plannedLocalGrowth += wordCount
        usedRegisters += (grownBase..grownLast)
        return grownBase
    }

    internal fun moveValue(dest: Int, src: Int, type: String) {
        val wide = registerWordCount(type) == 2
        val reference = isReferenceValueType(type)
        when {
            dest <= 15 && src <= 15 -> when {
                wide -> builder.moveWide(dest, src)
                reference -> builder.moveObject(dest, src)
                else -> builder.move(dest, src)
            }
            dest <= 0xFF -> when {
                wide -> builder.moveWideFrom16(dest, src)
                reference -> builder.moveObjectFrom16(dest, src)
                else -> builder.moveFrom16(dest, src)
            }
            else -> when {
                wide -> builder.moveWide16(dest, src)
                reference -> builder.moveObject16(dest, src)
                else -> builder.move16(dest, src)
            }
        }
    }

    internal fun lowRegister(value: ValueRefImpl): ValueRefImpl =
        value.asLowRegister()

    internal fun byteRegister(value: ValueRefImpl): ValueRefImpl =
        value.asByteRegister()

    private fun ValueRefImpl.asLowRegister(): ValueRefImpl {
        if (registerFits(register, wordCount, RegisterConstraint.LOW) && !isShiftedPhysicalRegister(register)) return this
        val dest = allocTemp(wordCount, RegisterConstraint.LOW)
        moveValue(dest, register, type)
        return ValueRefImpl(this@CodeEmitter, dest, type)
    }

    private fun ValueRefImpl.asByteRegister(): ValueRefImpl {
        if (registerFits(register, wordCount, RegisterConstraint.BYTE) && !isShiftedPhysicalRegister(register)) return this
        val dest = allocTemp(wordCount, RegisterConstraint.BYTE)
        moveValue(dest, register, type)
        return ValueRefImpl(this@CodeEmitter, dest, type)
    }

    internal fun registerWords(register: Int, wordCount: Int): List<Int> =
        if (isVirtualRegister(register)) {
            (0 until wordCount).map { virtualRegister(virtualRegisterId(register), it) }
        } else {
            (0 until wordCount).map { register + it }
        }

    private fun registerFits(register: Int, wordCount: Int, constraint: RegisterConstraint): Boolean {
        if (register == VOID_REGISTER) return false
        val base = resolvedRegisterForConstraintCheck(register)
        return base >= 0 && base + wordCount - 1 <= constraint.maxRegister
    }

    private fun resolvedRegisterForConstraintCheck(register: Int): Int {
        return if (isVirtualRegister(register)) {
            val allocation = tempAllocations[virtualRegisterId(register)] ?: return -1
            allocation.baseRegister + virtualRegisterOffset(register)
        } else if (!replaceMode && register >= originalIncomingBase) {
            register + plannedLocalGrowth
        } else {
            register
        }
    }

    private fun isShiftedPhysicalRegister(register: Int): Boolean =
        !replaceMode && register >= originalIncomingBase

    private fun resolveRegister(register: Int): Int {
        require(register != VOID_REGISTER) { "void ValueRef does not have a register" }
        if (isVirtualRegister(register)) {
            val allocation = tempAllocations[virtualRegisterId(register)]
                ?: error("unallocated virtual register $register")
            return allocation.baseRegister + virtualRegisterOffset(register)
        }
        return if (!replaceMode && register >= originalIncomingBase) {
            register + plannedLocalGrowth
        } else {
            register
        }
    }

    internal fun classFor(descriptor: String): DexClass =
        method.classDef.let {
            if (descriptor == it.info.descriptor) it
            else findClass(descriptor)?.let(::DexClass)
                ?: error("Class not found for descriptor $descriptor")
        }

    internal fun nextLabel(prefix: String): String = "${prefix}_${labelCounter++}"
}

internal enum class InvokeKind {
    STATIC,
    VIRTUAL,
    INTERFACE,
}

internal enum class ValueMoveKind {
    NARROW,
    WIDE,
    OBJECT,
}

internal fun registerWordCount(type: String): Int =
    if (type == "J" || type == "D") 2 else 1

private fun isReferenceValueType(type: String): Boolean =
    type.startsWith("L") || type.startsWith("[")

private const val VOID_REGISTER = Int.MIN_VALUE
private const val VIRTUAL_REGISTER_STRIDE = 0x10000

private fun virtualRegister(tempId: Int, offset: Int = 0): Int =
    -1 - (tempId * VIRTUAL_REGISTER_STRIDE + offset)

private fun isVirtualRegister(register: Int): Boolean =
    register < 0 && register != VOID_REGISTER

private fun virtualRegisterId(register: Int): Int =
    (-register - 1) / VIRTUAL_REGISTER_STRIDE

private fun virtualRegisterOffset(register: Int): Int =
    (-register - 1) % VIRTUAL_REGISTER_STRIDE

internal class AllocatingInstructionBuilder {
    private val ops = mutableListOf<Op>()

    private sealed interface Op
    private data class Label(val name: String) : Op
    private data object ReturnVoid : Op
    private data class Return(val reg: Int) : Op
    private data class ReturnObject(val reg: Int) : Op
    private data class ConstInt(val dest: Int, val value: Int) : Op
    private data class Const16(val dest: Int, val value: Int) : Op
    private data class ConstWide16(val dest: Int, val value: Long) : Op
    private data class ConstString(val dest: Int, val value: String) : Op
    private data class SgetObject(val dest: Int, val field: FieldRef) : Op
    private data class IgetObject(val dest: Int, val obj: Int, val field: FieldRef) : Op
    private data class Iget(val dest: Int, val obj: Int, val field: FieldRef) : Op
    private data class IgetWide(val dest: Int, val obj: Int, val field: FieldRef) : Op
    private data class IgetBoolean(val dest: Int, val obj: Int, val field: FieldRef) : Op
    private data class IgetByte(val dest: Int, val obj: Int, val field: FieldRef) : Op
    private data class IgetChar(val dest: Int, val obj: Int, val field: FieldRef) : Op
    private data class IgetShort(val dest: Int, val obj: Int, val field: FieldRef) : Op
    private data class InstanceOf(val dest: Int, val ref: Int, val descriptor: String) : Op
    private data class CheckCast(val reg: Int, val descriptor: String) : Op
    private data class Move(val dest: Int, val src: Int, val kind: ValueMoveKind) : Op
    private data class MoveResult(val dest: Int, val kind: ValueMoveKind) : Op
    private data class Invoke(val opcode: Int, val owner: String, val name: String, val proto: String, val registers: List<Int>) : Op
    private data class InvokeRange(val opcode: Int, val owner: String, val name: String, val proto: String, val startReg: Int, val regCount: Int) : Op
    private data class IfEqz(val reg: Int, val label: String) : Op
    private data class IfNez(val reg: Int, val label: String) : Op
    private data class IfNe(val left: Int, val right: Int, val label: String) : Op
    private data class Goto(val label: String) : Op
    private data class SubInt(val dest: Int, val left: Int, val right: Int) : Op

    fun label(name: String) {
        ops += Label(name)
    }

    fun returnVoid() {
        ops += ReturnVoid
    }

    fun return_(reg: Int) {
        ops += Return(reg)
    }

    fun returnObject(reg: Int) {
        ops += ReturnObject(reg)
    }

    fun constInt(dest: Int, value: Int) {
        ops += ConstInt(dest, value)
    }

    fun const4(dest: Int, value: Int) {
        ops += ConstInt(dest, value)
    }

    fun const16(dest: Int, value: Int) {
        ops += Const16(dest, value)
    }

    fun constWide16(dest: Int, value: Long) {
        ops += ConstWide16(dest, value)
    }

    fun constString(dest: Int, value: String) {
        ops += ConstString(dest, value)
    }

    fun sgetObject(dest: Int, field: FieldRef) {
        ops += SgetObject(dest, field)
    }

    fun igetObject(dest: Int, obj: Int, field: FieldRef) {
        ops += IgetObject(dest, obj, field)
    }

    fun iget(dest: Int, obj: Int, field: FieldRef) {
        ops += Iget(dest, obj, field)
    }

    fun igetWide(dest: Int, obj: Int, field: FieldRef) {
        ops += IgetWide(dest, obj, field)
    }

    fun igetBoolean(dest: Int, obj: Int, field: FieldRef) {
        ops += IgetBoolean(dest, obj, field)
    }

    fun igetByte(dest: Int, obj: Int, field: FieldRef) {
        ops += IgetByte(dest, obj, field)
    }

    fun igetChar(dest: Int, obj: Int, field: FieldRef) {
        ops += IgetChar(dest, obj, field)
    }

    fun igetShort(dest: Int, obj: Int, field: FieldRef) {
        ops += IgetShort(dest, obj, field)
    }

    fun instanceOf(dest: Int, ref: Int, descriptor: String) {
        ops += InstanceOf(dest, ref, descriptor)
    }

    fun checkCast(reg: Int, descriptor: String) {
        ops += CheckCast(reg, descriptor)
    }

    fun move(dest: Int, src: Int) {
        ops += Move(dest, src, ValueMoveKind.NARROW)
    }

    fun moveFrom16(dest: Int, src: Int) {
        move(dest, src)
    }

    fun move16(dest: Int, src: Int) {
        move(dest, src)
    }

    fun moveWide(dest: Int, src: Int) {
        ops += Move(dest, src, ValueMoveKind.WIDE)
    }

    fun moveWideFrom16(dest: Int, src: Int) {
        moveWide(dest, src)
    }

    fun moveWide16(dest: Int, src: Int) {
        moveWide(dest, src)
    }

    fun moveObject(dest: Int, src: Int) {
        ops += Move(dest, src, ValueMoveKind.OBJECT)
    }

    fun moveObjectFrom16(dest: Int, src: Int) {
        moveObject(dest, src)
    }

    fun moveObject16(dest: Int, src: Int) {
        moveObject(dest, src)
    }

    fun moveResult(dest: Int) {
        ops += MoveResult(dest, ValueMoveKind.NARROW)
    }

    fun moveResultWide(dest: Int) {
        ops += MoveResult(dest, ValueMoveKind.WIDE)
    }

    fun moveResultObject(dest: Int) {
        ops += MoveResult(dest, ValueMoveKind.OBJECT)
    }

    fun invokeStatic(owner: String, name: String, proto: String, vararg registers: Int) {
        ops += Invoke(Opcodes.INVOKE_STATIC, owner, name, proto, registers.toList())
    }

    fun invokeVirtual(owner: String, name: String, proto: String, vararg registers: Int) {
        ops += Invoke(Opcodes.INVOKE_VIRTUAL, owner, name, proto, registers.toList())
    }

    fun invokeInterface(owner: String, name: String, proto: String, vararg registers: Int) {
        ops += Invoke(Opcodes.INVOKE_INTERFACE, owner, name, proto, registers.toList())
    }

    fun invoke(opcode: Int, owner: String, name: String, proto: String, registers: List<Int>) {
        ops += Invoke(opcode, owner, name, proto, registers)
    }

    fun invokeRange(opcode: Int, owner: String, name: String, proto: String, startReg: Int, regCount: Int) {
        ops += InvokeRange(opcode, owner, name, proto, startReg, regCount)
    }

    fun ifEqz(reg: Int, label: String) {
        ops += IfEqz(reg, label)
    }

    fun ifNez(reg: Int, label: String) {
        ops += IfNez(reg, label)
    }

    fun ifNe(left: Int, right: Int, label: String) {
        ops += IfNe(left, right, label)
    }

    fun goto(label: String) {
        ops += Goto(label)
    }

    fun subInt(dest: Int, left: Int, right: Int) {
        ops += SubInt(dest, left, right)
    }

    fun build(resolve: (Int) -> Int): List<Instruction> {
        val builder = InstructionBuilder()
        for (op in ops) {
            when (op) {
                is Label -> builder.label(op.name)
                ReturnVoid -> builder.returnVoid()
                is Return -> builder.return_(resolveByteRegister(op.reg, resolve, "return"))
                is ReturnObject -> builder.returnObject(resolveByteRegister(op.reg, resolve, "return-object"))
                is ConstInt -> emitConstInt(builder, resolveByteRegister(op.dest, resolve, "const"), op.value)
                is Const16 -> builder.const16(resolveByteRegister(op.dest, resolve, "const/16"), op.value)
                is ConstWide16 -> builder.constWide16(resolveByteRegister(op.dest, resolve, "const-wide/16"), op.value)
                is ConstString -> builder.constString(resolveByteRegister(op.dest, resolve, "const-string"), op.value)
                is SgetObject -> builder.sgetObject(resolveByteRegister(op.dest, resolve, "sget-object"), op.field)
                is IgetObject -> builder.igetObject(
                    resolveLowRegister(op.dest, resolve, "iget-object A"),
                    resolveLowRegister(op.obj, resolve, "iget-object B"),
                    op.field,
                )
                is Iget -> builder.iget(
                    resolveLowRegister(op.dest, resolve, "iget A"),
                    resolveLowRegister(op.obj, resolve, "iget B"),
                    op.field,
                )
                is IgetWide -> builder.igetWide(
                    resolveLowRegister(op.dest, resolve, "iget-wide A"),
                    resolveLowRegister(op.obj, resolve, "iget-wide B"),
                    op.field,
                )
                is IgetBoolean -> builder.igetBoolean(
                    resolveLowRegister(op.dest, resolve, "iget-boolean A"),
                    resolveLowRegister(op.obj, resolve, "iget-boolean B"),
                    op.field,
                )
                is IgetByte -> builder.igetByte(
                    resolveLowRegister(op.dest, resolve, "iget-byte A"),
                    resolveLowRegister(op.obj, resolve, "iget-byte B"),
                    op.field,
                )
                is IgetChar -> builder.igetChar(
                    resolveLowRegister(op.dest, resolve, "iget-char A"),
                    resolveLowRegister(op.obj, resolve, "iget-char B"),
                    op.field,
                )
                is IgetShort -> builder.igetShort(
                    resolveLowRegister(op.dest, resolve, "iget-short A"),
                    resolveLowRegister(op.obj, resolve, "iget-short B"),
                    op.field,
                )
                is InstanceOf -> builder.instanceOf(
                    resolveLowRegister(op.dest, resolve, "instance-of A"),
                    resolveLowRegister(op.ref, resolve, "instance-of B"),
                    op.descriptor,
                )
                is CheckCast -> builder.checkCast(resolveByteRegister(op.reg, resolve, "check-cast"), op.descriptor)
                is Move -> emitMove(builder, op.kind, resolve(op.dest), resolve(op.src))
                is MoveResult -> emitMoveResult(builder, op.kind, resolveByteRegister(op.dest, resolve, "move-result"))
                is Invoke -> emitInvoke(builder, op, resolve)
                is InvokeRange -> emitInvokeRange(builder, op, resolve)
                is IfEqz -> builder.ifEqz(resolveByteRegister(op.reg, resolve, "if-eqz"), op.label)
                is IfNez -> builder.ifNez(resolveByteRegister(op.reg, resolve, "if-nez"), op.label)
                is IfNe -> builder.ifNe(
                    resolveLowRegister(op.left, resolve, "if-ne A"),
                    resolveLowRegister(op.right, resolve, "if-ne B"),
                    op.label,
                )
                is Goto -> builder.goto(op.label)
                is SubInt -> builder.add(
                    Instruction.Reg3(
                        Reg3Insn(
                            Opcodes.SUB_INT.toUShort(),
                            resolveByteRegister(op.dest, resolve, "sub-int A").toUShort(),
                            resolveByteRegister(op.left, resolve, "sub-int B").toUShort(),
                            resolveByteRegister(op.right, resolve, "sub-int C").toUShort(),
                        )
                    )
                )
            }
        }
        return builder.build()
    }

    private fun emitConstInt(builder: InstructionBuilder, dest: Int, value: Int) {
        when {
            dest <= 15 && value in -8..7 -> builder.const4(dest, value)
            value in Short.MIN_VALUE..Short.MAX_VALUE -> builder.const16(dest, value)
            else -> builder.const_(dest, value)
        }
    }

    private fun emitMove(builder: InstructionBuilder, kind: ValueMoveKind, dest: Int, src: Int) {
        when {
            dest <= 15 && src <= 15 -> when (kind) {
                ValueMoveKind.NARROW -> builder.move(dest, src)
                ValueMoveKind.WIDE -> builder.moveWide(dest, src)
                ValueMoveKind.OBJECT -> builder.moveObject(dest, src)
            }
            dest <= 0xFF -> when (kind) {
                ValueMoveKind.NARROW -> builder.moveFrom16(dest, src)
                ValueMoveKind.WIDE -> builder.moveWideFrom16(dest, src)
                ValueMoveKind.OBJECT -> builder.moveObjectFrom16(dest, src)
            }
            else -> when (kind) {
                ValueMoveKind.NARROW -> builder.move16(dest, src)
                ValueMoveKind.WIDE -> builder.moveWide16(dest, src)
                ValueMoveKind.OBJECT -> builder.moveObject16(dest, src)
            }
        }
    }

    private fun emitMoveResult(builder: InstructionBuilder, kind: ValueMoveKind, dest: Int) {
        when (kind) {
            ValueMoveKind.NARROW -> builder.moveResult(dest)
            ValueMoveKind.WIDE -> builder.moveResultWide(dest)
            ValueMoveKind.OBJECT -> builder.moveResultObject(dest)
        }
    }

    private fun emitInvoke(builder: InstructionBuilder, op: Invoke, resolve: (Int) -> Int) {
        val registers = op.registers.map(resolve).toIntArray()
        when (op.opcode) {
            Opcodes.INVOKE_STATIC -> builder.invokeStatic(op.owner, op.name, op.proto, *registers)
            Opcodes.INVOKE_VIRTUAL -> builder.invokeVirtual(op.owner, op.name, op.proto, *registers)
            Opcodes.INVOKE_INTERFACE -> builder.invokeInterface(op.owner, op.name, op.proto, *registers)
            else -> error("unsupported invoke opcode 0x${op.opcode.toString(16)}")
        }
    }

    private fun emitInvokeRange(builder: InstructionBuilder, op: InvokeRange, resolve: (Int) -> Int) {
        val startReg = resolveRangeStart(op.startReg, resolve)
        val regCount = resolveRangeCount(op.regCount)
        builder.add(
            Instruction.InvokeRange(
                InvokeRangeInsn(
                    op.opcode.toUShort(),
                    startReg.toUShort(),
                    regCount.toUShort(),
                    MethodRef(op.owner, op.name, op.proto),
                )
            )
        )
    }

    private fun resolveLowRegister(register: Int, resolve: (Int) -> Int, context: String): Int {
        val resolved = resolve(register)
        require(resolved in 0..15) { "$context requires a 4-bit register, got v$resolved" }
        return resolved
    }

    private fun resolveByteRegister(register: Int, resolve: (Int) -> Int, context: String): Int {
        val resolved = resolve(register)
        require(resolved in 0..0xFF) { "$context requires an 8-bit register, got v$resolved" }
        return resolved
    }

    private fun resolveRangeStart(register: Int, resolve: (Int) -> Int): Int {
        val resolved = resolve(register)
        require(resolved in 0..0xFFFF) { "invoke/range start requires a 16-bit register, got v$resolved" }
        return resolved
    }

    private fun resolveRangeCount(regCount: Int): Int {
        require(regCount in 0..0xFFFF) { "invoke/range count exceeds 16-bit limit: $regCount" }
        return regCount
    }
}

internal class ValueRefImpl(
    internal val emitter: CodeEmitter,
    val register: Int,
    overrideType: String,
) : ValueRef {
    val type: String = overrideType
    val wordCount: Int = registerWordCount(type)

    fun registerWords(): List<Int> =
        emitter.registerWords(register, wordCount)

    override fun cast(type: String): ValueRef =
        emitter.cast(this, type)

    override fun field(type: String): ValueRef =
        emitter.valueField(this, type)

    override fun field(ref: FieldHandle): ValueRef =
        emitter.valueField(this, ref.field)

    override fun virtualCall(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef =
        emitter.invoke(owner, name, proto, listOf(this) + args.map { it.asImpl() }, InvokeKind.VIRTUAL)

    override fun interfaceCall(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef =
        emitter.invoke(owner, name, proto, listOf(this) + args.map { it.asImpl() }, InvokeKind.INTERFACE)

    override fun size(): ValueRef =
        emitter.listSize(this)

    override fun get(index: ValueRef): ValueRef =
        emitter.listGet(this, index.asImpl())

    override fun minus(value: ValueRef): ValueRef =
        emitter.subtract(this, value.asImpl())
}

private fun ValueRef.asImpl(): ValueRefImpl =
    this as? ValueRefImpl ?: error("ValueRef is only valid inside CodeScope execution")

private class PointHandleImpl(
    override val debugName: String?,
    private val method: Method,
    private val index: Int,
    private val captures: List<CodeCapture>,
) : PointHandle {
    override fun insertBefore(block: CodeScope.() -> Unit) {
        method.insertCompiledCode(index, captures, block)
    }

    override fun insertAfter(block: CodeScope.() -> Unit) {
        val target = (index + 1).coerceAtMost(method.instructionCount)
        method.insertCompiledCode(target, captures, block)
    }
}

internal class MethodPointCompiler(
    private val handle: MethodHandleImpl,
    private val debug: String?,
) : PointQuery {
    private val method = handle.method
    private val instructions = method.instructions
    private val captures = mutableListOf<CodeCapture>()
    private var currentIndex = 0

    override fun checkCast(type: String) {
        var found = -1
        for (index in currentIndex until instructions.size) {
            val insn = instructions[index]
            if (insn is Instruction.RegType &&
                insn.opcode() == Opcodes.CHECK_CAST &&
                insn.value0.typeDescriptor == type
            ) {
                found = index
                break
            }
        }
        require(found >= 0) {
            "No CHECK_CAST to $type found in ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
        }
        captures += CodeCapture(type, method.registerA(found))
        currentIndex = found
    }

    override fun previousResult(type: String) {
        val found = instructions.subList(0, currentIndex).indices.reversed().firstOrNull { index ->
            val insn = instructions[index]
            when (insn.opcode()) {
                Opcodes.MOVE_RESULT,
                Opcodes.MOVE_RESULT_OBJECT,
                Opcodes.MOVE_RESULT_WIDE,
                -> {
                    val invoke = instructions.getOrNull(index - 1)?.methodRef()
                    invoke?.returnType == type
                }
                else -> false
            }
        } ?: error("No previous result of type $type before index $currentIndex")
        captures += CodeCapture(type, method.registerA(found))
        currentIndex = found
    }

    override fun nextBranch(opcode: String, afterInvokeOpcode: String?) {
        val branchOpcode = lookupOpcode(opcode)
        val afterOpcode = afterInvokeOpcode?.let(::lookupOpcode)
        var invokeSeen = afterOpcode == null
        var found = -1
        for (index in (currentIndex + 1) until instructions.size) {
            val insn = instructions[index]
            if (!invokeSeen && insn.opcode() == afterOpcode) {
                invokeSeen = true
            }
            if (invokeSeen && insn.opcode() == branchOpcode) {
                found = index
                break
            }
        }
        require(found >= 0) {
            "No branch $opcode found after index $currentIndex in ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
        }
        currentIndex = found
    }

    fun build(): PointHandle =
        PointHandleImpl(debug, method, currentIndex, captures.toList())
}

private fun lookupOpcode(name: String): Int =
    Opcodes::class.java.getField(name).getInt(null)

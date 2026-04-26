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
): List<Instruction> {
    val emitter = CodeEmitter.forInsertion(this, index, captures)
    emitter.block()
    return emitter.build()
}

internal fun Method.insertCodeBefore(block: CodeScope.() -> Unit) {
    insertInstructions(0, compileInsertedCode(0, emptyList(), block))
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
        insertInstructions(index, compileInsertedCode(index, captures, block))
    }
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

internal class CodeEmitter private constructor(
    internal val method: Method,
    private val insertIndex: Int?,
    private val replaceMode: Boolean,
    private val captures: List<CodeCapture>,
) : CodeScope {
    internal val builder = InstructionBuilder()
    private val usedRegisters = mutableSetOf<Int>()
    private var nextReplaceLocal = 0
    private var maxOutRegisters = 0
    private var labelCounter = 0

    companion object {
        fun forInsertion(method: Method, index: Int, captures: List<CodeCapture>): CodeEmitter =
            CodeEmitter(method, index, replaceMode = false, captures = captures)

        fun forReplacement(method: Method): CodeEmitter =
            CodeEmitter(method, insertIndex = null, replaceMode = true, captures = emptyList())
    }

    private val isStaticMethod: Boolean = (method.info.accessFlags.toInt() and ACC_STATIC) != 0
    private val incomingBase: Int = if (replaceMode) REPLACE_LOCAL_BUDGET else method.registersSize - method.insSize

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
        when {
            dest <= 15 && value in -8..7 -> builder.const4(dest, value)
            value in Short.MIN_VALUE..Short.MAX_VALUE -> builder.const16(dest, value)
            else -> builder.const_(dest, value)
        }
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

    internal fun build(): List<Instruction> = builder.build()

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
        val dest = allocTemp()
        val obj = value.asLowRegister()
        builder.igetObject(dest, obj.register, field)
        return ValueRefImpl(this, dest, field.fieldType)
    }

    internal fun invoke(owner: String, name: String, proto: String, args: List<ValueRefImpl>, invokeKind: InvokeKind): ValueRef {
        val registers = args.flatMap { it.registerWords() }
        maxOutRegisters = maxOf(maxOutRegisters, registers.size)
        when (invokeKind) {
            InvokeKind.STATIC -> builder.invokeStatic(owner, name, proto, *registers.toIntArray())
            InvokeKind.VIRTUAL -> builder.invokeVirtual(owner, name, proto, *registers.toIntArray())
            InvokeKind.INTERFACE -> builder.invokeInterface(owner, name, proto, *registers.toIntArray())
        }

        val returnType = proto.substringAfter(')')
        return if (returnType == "V") {
            ValueRefImpl(this, -1, "V")
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

    internal fun listSize(value: ValueRefImpl): ValueRef =
        invoke("Ljava/util/List;", "size", "()I", listOf(value), InvokeKind.INTERFACE)

    internal fun listGet(value: ValueRefImpl, index: ValueRefImpl): ValueRef =
        invoke("Ljava/util/List;", "get", "(I)Ljava/lang/Object;", listOf(value, index), InvokeKind.INTERFACE)

    internal fun subtract(left: ValueRefImpl, right: ValueRefImpl): ValueRef {
        val dest = allocTemp()
        val leftReg = left.asByteRegister().register
        val rightReg = right.asByteRegister().register
        builder.add(
            Instruction.Reg3(
                Reg3Insn(
                    Opcodes.SUB_INT.toUShort(),
                    dest.toUShort(),
                    leftReg.toUShort(),
                    rightReg.toUShort(),
                )
            )
        )
        return ValueRefImpl(this, dest, "I")
    }

    internal fun cast(value: ValueRefImpl, type: String): ValueRef {
        val target = value.asByteRegister()
        builder.checkCast(target.register, type)
        return ValueRefImpl(this, target.register, type)
    }

    internal fun allocTemp(wordCount: Int = 1): Int {
        require(wordCount > 0) { "wordCount must be positive" }
        if (replaceMode) {
            require(nextReplaceLocal + wordCount <= REPLACE_LOCAL_BUDGET) {
                "CodeScope exceeded local budget $REPLACE_LOCAL_BUDGET for ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
            }
            val register = nextReplaceLocal
            nextReplaceLocal += wordCount
            return register
        }

        val index = insertIndex ?: 0
        val registers = method.findContiguousFreeRegisters(index, wordCount, usedRegisters.toList())
        require(registers.size == wordCount && registers.all { it <= 15 }) {
            "CodeScope requires $wordCount free low scratch register(s) at ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}[$index], got $registers"
        }
        usedRegisters += registers
        val register = registers.first()
        return register
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
        if (register in 0..15) return this
        val dest = allocTemp(wordCount)
        moveValue(dest, register, type)
        return ValueRefImpl(this@CodeEmitter, dest, type)
    }

    private fun ValueRefImpl.asByteRegister(): ValueRefImpl {
        if (register in 0..0xFF) return this
        val dest = allocTemp(wordCount)
        moveValue(dest, register, type)
        return ValueRefImpl(this@CodeEmitter, dest, type)
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

internal fun registerWordCount(type: String): Int =
    if (type == "J" || type == "D") 2 else 1

private fun isReferenceValueType(type: String): Boolean =
    type.startsWith("L") || type.startsWith("[")

internal class ValueRefImpl(
    internal val emitter: CodeEmitter,
    val register: Int,
    overrideType: String,
) : ValueRef {
    val type: String = overrideType
    val wordCount: Int = registerWordCount(type)

    fun registerWords(): List<Int> =
        (0 until wordCount).map { register + it }

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
        method.insertInstructions(index, method.compileInsertedCode(index, captures, block))
    }

    override fun insertAfter(block: CodeScope.() -> Unit) {
        val target = (index + 1).coerceAtMost(method.instructionCount)
        method.insertInstructions(target, method.compileInsertedCode(target, captures, block))
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

// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.literal
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.dex.stringValue
import app.reseam.patch.dex.typeRef
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodRef

interface PointMatch {
    fun opcode(vararg opcodes: Opcode)

    fun string(value: String)

    fun stringContains(part: String)

    fun literal(value: Long)

    fun type(descriptor: String)

    fun checkCast(type: String)

    fun newInstance(type: String)

    fun invoke(vararg opcodes: Opcode, block: MethodRefMatch.() -> Unit = {})

    fun invokeStatic(block: MethodRefMatch.() -> Unit = {})

    fun invokeVirtual(block: MethodRefMatch.() -> Unit = {})

    fun invokeInterface(block: MethodRefMatch.() -> Unit = {})

    fun invokeDirect(block: MethodRefMatch.() -> Unit = {})

    /** An invoke of exactly this method. */
    fun calls(target: MethodTarget)

    fun field(block: FieldRefMatch.() -> Unit)

    /**
     * Every reaching source of an invoke argument must match; follows copies and exception paths.
     */
    fun argument(index: Int, block: PointMatch.() -> Unit)

    /** A `move-result` whose invoke returns `type`, or any when null. */
    fun resultOf(returns: String? = null)

    fun where(predicate: Instruction.() -> Boolean)

    /** The next step of the sequence, at most `within` instructions later. */
    fun then(within: Int = 1, block: PointMatch.() -> Unit)
}

interface MethodRefMatch {
    fun owner(type: String)

    /** The declared owner is this type or a known subtype of it. */
    fun ownerAssignableTo(type: String)

    fun name(value: String)

    fun returns(type: String)

    fun params(vararg types: String)

    fun hasParam(type: String)

    fun paramCount(count: Int)
}

interface FieldRefMatch {
    fun owner(type: String)

    fun name(value: String)

    fun type(descriptor: String)
}

internal typealias InstructionPredicate = (List<Instruction>, Int) -> Boolean

internal class PointStep(val predicates: List<InstructionPredicate>, val within: Int) {
    fun matches(insns: List<Instruction>, index: Int): Boolean = predicates.all { it(insns, index) }
}

internal fun findSequence(
    insns: List<Instruction>,
    steps: List<PointStep>,
    start: Int,
): List<Int>? {
    if (steps.isEmpty()) return null
    for (first in start until insns.size) {
        if (!steps[0].matches(insns, first)) continue
        val matched = mutableListOf(first)
        var ok = true
        for (step in steps.drop(1)) {
            val from = matched.last() + 1
            val hit =
                (from until minOf(insns.size, from + step.within)).firstOrNull {
                    step.matches(insns, it)
                }
            if (hit == null) {
                ok = false
                break
            }
            matched += hit
        }
        if (ok) return matched
    }
    return null
}

internal class PointMatchSpec(private val method: Method) : PointMatch {
    private val origins by lazy { ArgumentOrigins(method) }
    val diagnostics: List<String>
        get() = if (usesOrigins) origins.diagnostics.toList() else emptyList()

    private var usesOrigins = false
    var startsWithResult = false
        private set

    private val built = mutableListOf<PointStep>()
    private var current = mutableListOf<InstructionPredicate>()
    private var currentWithin = 1

    fun steps(): List<PointStep> = built + PointStep(current.toList(), currentWithin)

    private fun add(predicate: InstructionPredicate) {
        current += predicate
    }

    private fun insn(predicate: Instruction.() -> Boolean) = add { insns, i ->
        insns[i].predicate()
    }

    override fun opcode(vararg opcodes: Opcode) = insn { opcode in opcodes }

    override fun string(value: String) = insn { stringValue == value }

    override fun stringContains(part: String) = insn { stringValue?.contains(part) == true }

    override fun literal(value: Long) = insn { literal == value }

    override fun type(descriptor: String) {
        val wanted = descriptor(descriptor)
        insn { typeRef == wanted }
    }

    override fun checkCast(type: String) {
        opcode(Opcode.CHECK_CAST)
        type(type)
    }

    override fun newInstance(type: String) {
        opcode(Opcode.NEW_INSTANCE)
        type(type)
    }

    override fun invoke(vararg opcodes: Opcode, block: MethodRefMatch.() -> Unit) {
        val match = MethodRefMatchSpec().apply(block)
        insn {
            (opcodes.isEmpty() && opcode?.isInvoke == true || opcode in opcodes) &&
                methodRef?.let(match::matches) == true
        }
    }

    override fun invokeStatic(block: MethodRefMatch.() -> Unit) =
        invoke(Opcode.INVOKE_STATIC, Opcode.INVOKE_STATIC_RANGE, block = block)

    override fun invokeVirtual(block: MethodRefMatch.() -> Unit) =
        invoke(Opcode.INVOKE_VIRTUAL, Opcode.INVOKE_VIRTUAL_RANGE, block = block)

    override fun invokeInterface(block: MethodRefMatch.() -> Unit) =
        invoke(Opcode.INVOKE_INTERFACE, Opcode.INVOKE_INTERFACE_RANGE, block = block)

    override fun invokeDirect(block: MethodRefMatch.() -> Unit) =
        invoke(Opcode.INVOKE_DIRECT, Opcode.INVOKE_DIRECT_RANGE, block = block)

    override fun calls(target: MethodTarget) = insn {
        methodRef?.let {
            it.definingClass == target.owner && it.name == target.name && it.proto == target.proto
        } == true
    }

    override fun field(block: FieldRefMatch.() -> Unit) {
        val match = FieldRefMatchSpec().apply(block)
        insn { fieldRef?.let(match::matches) == true }
    }

    override fun argument(index: Int, block: PointMatch.() -> Unit) {
        require(index >= 0) { "Argument index must be nonnegative" }
        val source = PointMatchSpec(method).apply(block)
        val step =
            source.steps().singleOrNull()
                ?: error("argument {} matches a source instruction, not a sequence")
        usesOrigins = true
        add { insns, i -> origins.matches(insns, i, index, step) }
    }

    override fun resultOf(returns: String?) {
        if (built.isEmpty()) startsWithResult = true
        val wanted = returns?.let(::descriptor)
        add { insns, i ->
            insns[i].opcode?.isMoveResult == true &&
                insns.getOrNull(i - 1)?.methodRef?.let {
                    wanted == null || it.returnType == wanted
                } == true
        }
    }

    override fun where(predicate: Instruction.() -> Boolean) = insn(predicate)

    override fun then(within: Int, block: PointMatch.() -> Unit) {
        built += PointStep(current.toList(), currentWithin)
        current = mutableListOf()
        currentWithin = within
        block()
    }
}

internal class MethodRefMatchSpec : MethodRefMatch {
    var owner: String? = null
        private set

    var name: String? = null
        private set

    var returnType: String? = null
        private set

    var parameters: List<String>? = null
        private set

    private val assignableOwners = mutableListOf<String>()
    private val required = mutableListOf<String>()
    val requiredParameters: List<String>
        get() = required

    var parameterCount: Int? = null
        private set

    val needsPostFilter
        get() = assignableOwners.isNotEmpty()

    override fun owner(type: String) {
        owner = descriptor(type)
    }

    override fun ownerAssignableTo(type: String) {
        assignableOwners += descriptor(type)
    }

    override fun name(value: String) {
        name = value
    }

    override fun returns(type: String) {
        returnType = descriptor(type)
    }

    override fun params(vararg types: String) {
        parameters = types.map(::descriptor)
    }

    override fun hasParam(type: String) {
        required += descriptor(type)
    }

    override fun paramCount(count: Int) {
        parameterCount = count
    }

    fun matches(ref: MethodRef): Boolean {
        if (owner != null && ref.definingClass != owner || name != null && ref.name != name)
            return false
        if (returnType != null && ref.returnType != returnType) return false
        if (assignableOwners.any { !isAssignableType(ref.definingClass, it) }) return false
        if (parameters != null || requiredParameters.isNotEmpty() || parameterCount != null) {
            val actual = ref.parameterTypes
            if (
                parameters != null && actual != parameters ||
                    parameterCount != null && actual.size != parameterCount
            )
                return false
            if (requiredParameters.any { it !in actual }) return false
        }
        return true
    }
}

internal class FieldRefMatchSpec : FieldRefMatch {
    private val checks = mutableListOf<(FieldRef) -> Boolean>()

    override fun owner(type: String) {
        val wanted = descriptor(type)
        checks += { it.definingClass == wanted }
    }

    override fun name(value: String) {
        checks += { it.name == value }
    }

    override fun type(descriptor: String) {
        val wanted = descriptor(descriptor)
        checks += { it.fieldType == wanted }
    }

    fun matches(ref: FieldRef) = checks.all { it(ref) }
}

internal fun inferType(method: Method, index: Int): String? {
    val insns = method.instructions
    val insn = insns[index]
    return when (insn.opcode) {
        Opcode.CHECK_CAST,
        Opcode.NEW_INSTANCE,
        Opcode.NEW_ARRAY -> insn.typeRef
        Opcode.CONST_STRING,
        Opcode.CONST_STRING_JUMBO -> Type.String
        Opcode.CONST_CLASS -> "Ljava/lang/Class;"
        Opcode.INSTANCE_OF,
        Opcode.ARRAY_LENGTH -> Type.Int
        Opcode.MOVE_RESULT,
        Opcode.MOVE_RESULT_WIDE,
        Opcode.MOVE_RESULT_OBJECT -> insns.getOrNull(index - 1)?.methodRef?.returnType
        Opcode.MOVE_EXCEPTION -> "Ljava/lang/Throwable;"
        Opcode.IGET,
        Opcode.IGET_WIDE,
        Opcode.IGET_OBJECT,
        Opcode.IGET_BOOLEAN,
        Opcode.IGET_BYTE,
        Opcode.IGET_CHAR,
        Opcode.IGET_SHORT,
        Opcode.SGET,
        Opcode.SGET_WIDE,
        Opcode.SGET_OBJECT,
        Opcode.SGET_BOOLEAN,
        Opcode.SGET_BYTE,
        Opcode.SGET_CHAR,
        Opcode.SGET_SHORT -> insn.fieldRef?.fieldType
        else -> null
    }
}

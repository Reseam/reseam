// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.literal
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.regA
import app.reseam.patch.dex.returnType
import app.reseam.patch.dex.stringValue
import app.reseam.patch.dex.typeRef

/** One instruction in a method, found by matching, with the values captured on the way. */
class PointTarget internal constructor(
    debugName: String?,
    val method: MethodTarget,
    private val resolver: (PatchRuntime) -> Resolution<ResolvedPoint>,
) : Target<ResolvedPoint>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<ResolvedPoint> = resolver(runtime)

    val index: Int get() = resolved.index

    /** The instruction at the point. */
    val instruction: Instruction get() = resolved.method.instructions[resolved.index]

    /** The nearest instruction before this one matching the block. */
    fun previous(block: PointMatch.() -> Unit): PointTarget = derive("previous", block) { insns, from, steps ->
        val step = steps.singleOrNull() ?: error("previous {} takes a single step; use next {} for sequences")
        (from - 1 downTo 0).firstOrNull { step.matches(insns, it) }
    }

    /** The nearest instruction after this one matching the block, a sequence ending at its last step. */
    fun next(block: PointMatch.() -> Unit): PointTarget = derive("next", block) { insns, from, steps ->
        findSequence(insns, steps, from + 1)?.last()
    }

    /** Captures the register this instruction writes, for `capture(name)` in code emitted at a later point. */
    fun captureAs(name: String, type: String? = null): PointTarget {
        val base = this
        return PointTarget(debugName, method) { runtime ->
            val point = runtime.resolve(base).value
            val insn = point.method.instructions[point.index]
            val register = insn.regA ?: error("${base.label}: instruction ${point.index} (${insn.opcode}) writes no register to capture")
            val captureType = type?.let(::descriptor) ?: inferType(point.method, point.index)
                ?: error("${base.label}: cannot infer the type of the value at instruction ${point.index} (${insn.opcode}); pass captureAs(\"$name\", type)")
            Resolution(point.copy(captures = point.captures + Capture(name, captureType, register)), runtime.resolve(base).report)
        }
    }

    /** The method invoked at this point, as a target. */
    fun callee(debugName: String? = null): MethodTarget {
        val base = this
        return MethodTarget(debugName ?: "${base.label}.callee") { runtime ->
            val point = runtime.resolve(base).value
            val ref = point.method.instructions[point.index].methodRef
                ?: error("${base.label}: instruction ${point.index} is not an invoke")
            val method = runtime.index.methodFor(ref) ?: error("${base.label}: callee ${ref.definingClass}->${ref.name}${ref.proto} is not in the app")
            Resolution(method, wrapped(debugName ?: "${base.label}.callee", method.descriptor))
        }
    }

    /** The field accessed at this point, as a target. */
    fun field(debugName: String? = null): FieldTarget {
        val base = this
        return FieldTarget(debugName ?: "${base.label}.field") { runtime ->
            val point = runtime.resolve(base).value
            val ref = point.method.instructions[point.index].fieldRef
                ?: error("${base.label}: instruction ${point.index} is not a field access")
            Resolution(ref, wrapped(debugName ?: "${base.label}.field", "${ref.definingClass}.${ref.name}:${ref.fieldType}"))
        }
    }

    private fun derive(kind: String, block: PointMatch.() -> Unit, find: (List<Instruction>, Int, List<PointStep>) -> Int?): PointTarget {
        val base = this
        return PointTarget(debugName, method) { runtime ->
            val point = runtime.resolve(base).value
            val insns = point.method.instructions
            val index = find(insns, point.index, PointMatchSpec().apply(block).steps())
                ?: error("${base.label}: no instruction matched $kind {} from index ${point.index} in ${point.method.descriptor}")
            Resolution(point.copy(index = index), wrapped(base.label, "${point.method.descriptor}[$index]"))
        }
    }
}

/** The instruction in the method matching the block; a sequence resolves to its last step. */
fun MethodTarget.point(debugName: String? = null, block: PointMatch.() -> Unit): PointTarget {
    val owner = this
    val label = debugName ?: "${owner.label}.point"
    return PointTarget(label, owner) { runtime ->
        val spec = PointMatchSpec().apply(block)
        val method = runtime.resolve(owner).value
        val matched = findSequence(method.instructions, spec.steps(), 0)
            ?: error("$label: no instruction sequence matched in ${method.descriptor}")
        Resolution(ResolvedPoint(method, matched.last(), emptyList()), wrapped(label, "${method.descriptor}[${matched.last()}]"))
    }
}

class Capture internal constructor(val name: String, val type: String, val register: Int)

/** A resolved [PointTarget]: the method, the instruction index, and the captures collected so far. */
class ResolvedPoint internal constructor(val method: Method, val index: Int, internal val captures: List<Capture>) {
    internal fun copy(index: Int = this.index, captures: List<Capture> = this.captures) = ResolvedPoint(method, index, captures)
}

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
    /** A `move-result` whose invoke returns `type`, or any when null. */
    fun resultOf(returns: String? = null)
    fun where(predicate: Instruction.() -> Boolean)
    /** The next step of the sequence, at most `within` instructions later. */
    fun then(within: Int = 1, block: PointMatch.() -> Unit)
}

interface MethodRefMatch {
    fun owner(type: String)
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

internal fun findSequence(insns: List<Instruction>, steps: List<PointStep>, start: Int): List<Int>? {
    if (steps.isEmpty()) return null
    for (first in start until insns.size) {
        if (!steps[0].matches(insns, first)) continue
        val matched = mutableListOf(first)
        var ok = true
        for (step in steps.drop(1)) {
            val from = matched.last() + 1
            val hit = (from until minOf(insns.size, from + step.within)).firstOrNull { step.matches(insns, it) }
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

internal class PointMatchSpec : PointMatch {
    private val built = mutableListOf<PointStep>()
    private var current = mutableListOf<InstructionPredicate>()
    private var currentWithin = 1

    fun steps(): List<PointStep> = built + PointStep(current.toList(), currentWithin)

    private fun add(predicate: InstructionPredicate) {
        current += predicate
    }

    private fun insn(predicate: Instruction.() -> Boolean) = add { insns, i -> insns[i].predicate() }

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
        insn { (opcodes.isEmpty() && opcode?.isInvoke == true || opcode in opcodes) && methodRef?.let(match::matches) == true }
    }
    override fun invokeStatic(block: MethodRefMatch.() -> Unit) = invoke(Opcode.INVOKE_STATIC, Opcode.INVOKE_STATIC_RANGE, block = block)
    override fun invokeVirtual(block: MethodRefMatch.() -> Unit) = invoke(Opcode.INVOKE_VIRTUAL, Opcode.INVOKE_VIRTUAL_RANGE, block = block)
    override fun invokeInterface(block: MethodRefMatch.() -> Unit) = invoke(Opcode.INVOKE_INTERFACE, Opcode.INVOKE_INTERFACE_RANGE, block = block)
    override fun invokeDirect(block: MethodRefMatch.() -> Unit) = invoke(Opcode.INVOKE_DIRECT, Opcode.INVOKE_DIRECT_RANGE, block = block)
    override fun calls(target: MethodTarget) = insn {
        methodRef?.let { it.definingClass == target.owner && it.name == target.name && it.proto == target.proto } == true
    }
    override fun field(block: FieldRefMatch.() -> Unit) {
        val match = FieldRefMatchSpec().apply(block)
        insn { fieldRef?.let(match::matches) == true }
    }
    override fun resultOf(returns: String?) {
        val wanted = returns?.let(::descriptor)
        add { insns, i ->
            insns[i].opcode?.isMoveResult == true &&
                insns.getOrNull(i - 1)?.methodRef?.let { wanted == null || it.returnType == wanted } == true
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
    private val checks = mutableListOf<(MethodRef) -> Boolean>()
    override fun owner(type: String) { val wanted = descriptor(type); checks += { it.definingClass == wanted } }
    override fun name(value: String) { checks += { it.name == value } }
    override fun returns(type: String) { val wanted = descriptor(type); checks += { it.returnType == wanted } }
    override fun params(vararg types: String) { val wanted = types.map(::descriptor); checks += { it.parameterTypes == wanted } }
    override fun hasParam(type: String) { val wanted = descriptor(type); checks += { wanted in it.parameterTypes } }
    override fun paramCount(count: Int) { checks += { it.parameterTypes.size == count } }
    fun matches(ref: MethodRef) = checks.all { it(ref) }
}

internal class FieldRefMatchSpec : FieldRefMatch {
    private val checks = mutableListOf<(FieldRef) -> Boolean>()
    override fun owner(type: String) { val wanted = descriptor(type); checks += { it.definingClass == wanted } }
    override fun name(value: String) { checks += { it.name == value } }
    override fun type(descriptor: String) { val wanted = descriptor(descriptor); checks += { it.fieldType == wanted } }
    fun matches(ref: FieldRef) = checks.all { it(ref) }
}

/** The static type of the value an instruction writes to its first register. */
private fun inferType(method: Method, index: Int): String? {
    val insns = method.instructions
    val insn = insns[index]
    return when (insn.opcode) {
        Opcode.CHECK_CAST, Opcode.NEW_INSTANCE, Opcode.NEW_ARRAY -> insn.typeRef
        Opcode.CONST_STRING, Opcode.CONST_STRING_JUMBO -> Type.String
        Opcode.CONST_CLASS -> "Ljava/lang/Class;"
        Opcode.INSTANCE_OF, Opcode.ARRAY_LENGTH -> Type.Int
        Opcode.MOVE_RESULT, Opcode.MOVE_RESULT_WIDE, Opcode.MOVE_RESULT_OBJECT -> insns.getOrNull(index - 1)?.methodRef?.returnType
        Opcode.MOVE_EXCEPTION -> "Ljava/lang/Throwable;"
        Opcode.IGET, Opcode.IGET_WIDE, Opcode.IGET_OBJECT, Opcode.IGET_BOOLEAN, Opcode.IGET_BYTE, Opcode.IGET_CHAR, Opcode.IGET_SHORT,
        Opcode.SGET, Opcode.SGET_WIDE, Opcode.SGET_OBJECT, Opcode.SGET_BOOLEAN, Opcode.SGET_BYTE, Opcode.SGET_CHAR, Opcode.SGET_SHORT,
        -> insn.fieldRef?.fieldType
        else -> null
    }
}

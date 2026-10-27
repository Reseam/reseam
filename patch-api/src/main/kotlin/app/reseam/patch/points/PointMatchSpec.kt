// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.points

import app.reseam.patch.FieldRefMatch
import app.reseam.patch.MethodRefMatch
import app.reseam.patch.MethodTarget
import app.reseam.patch.PointMatch
import app.reseam.patch.descriptor
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.literal
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.returnType
import app.reseam.patch.dex.stringValue
import app.reseam.patch.dex.typeRef
import app.reseam.patch.proto
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.search.FieldRefMatchSpec
import app.reseam.patch.search.MethodRefMatchSpec
import app.reseam.patch.types.Instruction

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
    private val references
        get() = ActiveRuntime.current.index.references

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
        val pattern = MethodRefMatchSpec().apply(block).pattern
        add { insns, i ->
            val opcode = insns[i].opcode
            (opcodes.isEmpty() && opcode?.isInvoke == true || opcode in opcodes) &&
                i in references.callsIn(method, pattern)
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
        val pattern = FieldRefMatchSpec().apply(block).pattern
        add { _, i -> i in references.accessesIn(method, pattern) }
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

// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.invokeRegisters
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.referencedRegisters
import app.reseam.patch.dex.regA
import app.reseam.patch.types.Instruction

private enum class PointDirection(val label: String) {
    PREVIOUS("previous"),
    NEXT("next"),
}

/** One instruction in a method, found by matching, with the values captured on the way. */
class PointTarget
internal constructor(
    debugName: String?,
    val method: MethodTarget,
    private val resolver: (PatchRuntime) -> Resolution<ResolvedPoint>,
) : Target<ResolvedPoint>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<ResolvedPoint> = resolver(runtime)

    /** The instruction's index now, moved along by every edit the method has had since. */
    val index: Int
        get() = resolved.index

    /** The instruction at the point. */
    val instruction: Instruction
        get() = resolved.method.instructions[resolved.index]

    /** The nearest instruction before this one matching the block. */
    fun previous(block: PointMatch.() -> Unit): PointTarget =
        derive(PointDirection.PREVIOUS, block) { insns, from, steps ->
            val step =
                steps.singleOrNull()
                    ?: error("previous {} takes a single step; use next {} for sequences")
            (from - 1 downTo 0).firstOrNull { step.matches(insns, it) }
        }

    /**
     * The nearest instruction after this one matching the block, a sequence ending at its last
     * step.
     */
    fun next(block: PointMatch.() -> Unit): PointTarget =
        derive(PointDirection.NEXT, block) { insns, from, steps ->
            findSequence(insns, steps, from + 1)?.last()
        }

    /**
     * Captures the register this instruction writes, for `capture(name)` in code emitted at a later
     * point.
     */
    fun captureAs(name: String, type: String? = null): PointTarget {
        val base = this
        return PointTarget(debugName, method) { runtime ->
            val point = runtime.resolve(base).value
            val insn = point.method.instructions[point.index]
            val register = insn.regA ?: error(noRegisterMessage(base.label, point.index, insn))
            val captureType =
                type?.let(::descriptor)
                    ?: inferType(point.method, point.index)
                    ?: error(
                        "${base.label}: cannot infer the type of the value at instruction ${point.index} (${insn.opcode}); pass captureAs(\"$name\", type)"
                    )
            val capture =
                Capture(name, captureType, register).also { runtime.edits.track(point.method, it) }
            Resolution(point.withCaptures(point.captures + capture), runtime.resolve(base).report)
        }
    }

    /**
     * Captures argument [argument] of the invoke for `capture(name)`. Argument 0 of an instance
     * invoke is the receiver; a wide argument counts once.
     */
    fun captureArgumentAs(name: String, argument: Int, type: String? = null): PointTarget {
        val base = this
        return PointTarget(debugName, method) { runtime ->
            val point = runtime.resolve(base).value
            val value =
                ArgumentOrigins(point.method)
                    .atEntry(
                        point.method.instructions,
                        point.index,
                        argument,
                        point.anchor.head,
                        base.label,
                    )
            val register = value.register
            val argumentType = value.type
            val capture =
                Capture(name, type?.let(::descriptor) ?: argumentType, register).also {
                    runtime.edits.track(point.method, it)
                }
            Resolution(point.withCaptures(point.captures + capture), runtime.resolve(base).report)
        }
    }

    /**
     * Finds the unique instruction defining argument [argument], including exception paths.
     * Register-to-register moves are followed to the instruction that produced the value. Fails for
     * multiple reaching definitions or an incoming parameter. Arguments are counted as in
     * [captureArgumentAs].
     */
    fun writer(argument: Int, debugName: String? = null): PointTarget {
        val base = this
        val label = debugName ?: "${base.label}.writer($argument)"
        return PointTarget(label, method) { runtime ->
            val point = runtime.resolve(base).value
            val insns = point.method.instructions
            val origins =
                requireNotNull(
                    ArgumentOrigins(point.method).argument(insns, point.index, argument)
                ) {
                    "$label: invoke has no argument $argument"
                }
            val register = insns[point.index].arguments(label)[argument].register
            val index =
                origins.sites.singleOrNull()?.takeUnless { origins.entry || origins.unknown }
                    ?: error(writerMessage(label, point.method, point.index, register, origins))
            Resolution(
                point.at(runtime, label, index),
                wrapped(label, "${point.method.descriptor}[$index]"),
            )
        }
    }

    /** The method invoked at this point, as a target. */
    fun callee(debugName: String? = null): MethodTarget {
        val base = this
        return MethodTarget(debugName ?: "${base.label}.callee") { runtime ->
            val point = runtime.resolve(base).value
            val ref =
                point.method.instructions[point.index].methodRef
                    ?: error("${base.label}: instruction ${point.index} is not an invoke")
            val method =
                runtime.index.methodFor(ref)
                    ?: error(
                        "${base.label}: callee ${ref.definingClass}->${ref.name}${ref.proto} is not in the app"
                    )
            Resolution(method, wrapped(debugName ?: "${base.label}.callee", method.descriptor))
        }
    }

    /** The field accessed at this point, as a target. */
    fun field(debugName: String? = null): FieldTarget {
        val base = this
        return FieldTarget(debugName ?: "${base.label}.field") { runtime ->
            val point = runtime.resolve(base).value
            val ref =
                point.method.instructions[point.index].fieldRef
                    ?: error("${base.label}: instruction ${point.index} is not a field access")
            Resolution(
                ref,
                wrapped(
                    debugName ?: "${base.label}.field",
                    "${ref.definingClass}.${ref.name}:${ref.fieldType}",
                ),
            )
        }
    }

    private fun derive(
        direction: PointDirection,
        block: PointMatch.() -> Unit,
        find: (List<Instruction>, Int, List<PointStep>) -> Int?,
    ): PointTarget {
        val base = this
        return PointTarget(debugName, method) { runtime ->
            val point = runtime.resolve(base).value
            val insns = point.method.instructions
            val spec = PointMatchSpec(point.method).apply(block)
            val steps = spec.steps()
            val index =
                (if (
                    direction == PointDirection.NEXT &&
                        spec.startsWithResult &&
                        insns[point.index].opcode?.isInvoke == true
                ) {
                    findSequence(insns, steps, point.index + 1)
                        ?.takeIf { it.first() == point.index + 1 }
                        ?.last()
                } else find(insns, point.index, steps))
                    ?: error(
                        "${base.label}: no instruction matched ${direction.label} {} from index ${point.index} in ${point.method.descriptor}"
                    )
            Resolution(
                point.at(runtime, base.label, index),
                wrapped(base.label, "${point.method.descriptor}[$index]"),
            )
        }
    }
}

/** The instruction in the method matching the block; a sequence resolves to its last step. */
fun MethodTarget.point(debugName: String? = null, block: PointMatch.() -> Unit): PointTarget {
    val owner = this
    val label = debugName ?: "${owner.label}.point"
    return PointTarget(label, owner) { runtime ->
        val method = runtime.resolve(owner).value
        val spec = PointMatchSpec(method).apply(block)
        val matched =
            findSequence(method.instructions, spec.steps(), 0)
                ?: error("$label: no instruction sequence matched in ${method.descriptor}")
        val anchor = runtime.edits.anchor(method, matched.last(), label)
        Resolution(
            ResolvedPoint(method, anchor, emptyList()),
            wrapped(label, "${method.descriptor}[${matched.last()}]"),
        )
    }
}

class Capture internal constructor(val name: String, val type: String, register: Int) {
    /** Growing a method's frame renumbers its registers, so a capture follows its own. */
    var register: Int = register
        private set

    internal fun relocate(base: Int, additional: Int) {
        if (register >= base) register += additional
    }
}

/**
 * A resolved [PointTarget]: the method, the instruction it anchors to, and the captures collected
 * so far.
 */
class ResolvedPoint
internal constructor(
    val method: Method,
    internal val anchor: InstructionAnchor,
    internal val captures: List<Capture>,
) {
    val index: Int
        get() = anchor.instruction

    internal fun withCaptures(captures: List<Capture>) = ResolvedPoint(method, anchor, captures)

    internal fun at(runtime: PatchRuntime, label: String, index: Int) =
        ResolvedPoint(method, runtime.edits.anchor(method, index, label), captures)
}

private fun writerMessage(
    label: String,
    method: Method,
    index: Int,
    register: Int,
    writers: ArgumentOrigins.Origins,
): String {
    val where = "v$register at ${method.descriptor}[$index]"
    val sites = writers.sites.joinToString { "[$it] ${method.instructions[it].opcode}" }
    return when {
        writers.unknown ->
            "$label: $where has unknown control flow, cyclic copies or inconsistent wide sources"
        writers.sites.isEmpty() && writers.entry ->
            "$label: $where is a value the method was passed, not one an instruction of it wrote"
        writers.sites.isEmpty() -> "$label: nothing reaches $where; the instruction is unreachable"
        writers.entry ->
            "$label: $where is either what the method was passed or one of $sites, depending on the path"
        else -> "$label: $where is written on more than one path in: $sites"
    }
}

private fun noRegisterMessage(label: String, index: Int, insn: Instruction): String =
    when {
        insn.invokeRegisters != null ->
            "$label: instruction $index (${insn.opcode}) writes no register; capture one of its arguments with captureArgumentAs(name, argument)"
        insn.referencedRegisters.isEmpty() ->
            "$label: instruction $index (${insn.opcode}) uses no register at all, so there is nothing to capture"
        else -> "$label: instruction $index (${insn.opcode}) writes no register to capture"
    }

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.arguments
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.regA
import app.reseam.patch.points.ArgumentOrigins
import app.reseam.patch.points.PointMatchSpec
import app.reseam.patch.points.PointStep
import app.reseam.patch.points.findSequence
import app.reseam.patch.points.inferType
import app.reseam.patch.points.noRegisterMessage
import app.reseam.patch.points.writerMessage
import app.reseam.patch.run.Resolution
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
        get() = resolved.instruction

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
    fun captureAs(name: String, type: String? = null): PointTarget = capturing { point ->
        val insn = point.instruction
        val register = insn.regA ?: error(noRegisterMessage(label, point.index, insn))
        val captureType =
            type?.let(::descriptor)
                ?: inferType(point.method, point.index)
                ?: error(
                    "$label: cannot infer the type of the value at instruction ${point.index} (${insn.opcode}); pass captureAs(\"$name\", type)"
                )
        Capture(name, captureType, register)
    }

    /**
     * Captures argument [argument] of the invoke for `capture(name)`. Argument 0 of an instance
     * invoke is the receiver; a wide argument counts once.
     */
    fun captureArgumentAs(name: String, argument: Int, type: String? = null): PointTarget =
        capturing { point ->
            val value =
                ArgumentOrigins(point.method)
                    .atEntry(
                        point.method.instructions,
                        point.index,
                        argument,
                        point.anchor.head,
                        label,
                    )
            Capture(name, type?.let(::descriptor) ?: value.type, value.register)
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
            point.at(runtime, label, index).resolution(label)
        }
    }

    /** The method invoked at this point, as a target. */
    fun callee(debugName: String? = null): MethodTarget {
        val base = this
        return MethodTarget(debugName ?: "${base.label}.callee") { runtime ->
            val point = runtime.resolve(base).value
            val ref =
                point.instruction.methodRef
                    ?: error("${base.label}: instruction ${point.index} is not an invoke")
            val method =
                runtime.index.methodFor(ref)
                    ?: error(
                        "${base.label}: callee ${ref.definingClass}->${ref.name}${ref.proto} is not in the app"
                    )
            Resolution.direct(method, debugName ?: "${base.label}.callee", method.descriptor)
        }
    }

    /** The field accessed at this point, as a target. */
    fun field(debugName: String? = null): FieldTarget {
        val base = this
        return FieldTarget(debugName ?: "${base.label}.field") { runtime ->
            val point = runtime.resolve(base).value
            val ref =
                point.instruction.fieldRef
                    ?: error("${base.label}: instruction ${point.index} is not a field access")
            resolvedField(debugName ?: "${base.label}.field", ref)
        }
    }

    private fun capturing(capture: (ResolvedPoint) -> Capture): PointTarget {
        val base = this
        return PointTarget(debugName, method) { runtime ->
            val point = runtime.resolve(base).value
            val captured = capture(point).also { runtime.edits.track(point.method, it) }
            Resolution(point.withCaptures(point.captures + captured), runtime.resolve(base).report)
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
            point.at(runtime, base.label, index).resolution(base.label)
        }
    }
}

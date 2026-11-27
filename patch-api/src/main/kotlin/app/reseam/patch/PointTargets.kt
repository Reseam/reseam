// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.jumps
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.operands
import app.reseam.patch.dex.regA
import app.reseam.patch.dex.writers
import app.reseam.patch.native.jumpSources
import app.reseam.patch.points.ArgumentOrigins
import app.reseam.patch.points.PointMatchSpec
import app.reseam.patch.points.PointStep
import app.reseam.patch.points.findSequence
import app.reseam.patch.points.noRegisterMessage
import app.reseam.patch.points.registerAType
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
     * Captures the value in this instruction's first register, for `capture(name)` in code emitted
     * at a later point: the value it writes, or the value a store, a branch or a return takes
     * there.
     */
    fun captureAs(name: String, type: String? = null): PointTarget = capturing { point ->
        val insn = point.instruction
        val register = insn.regA ?: error(noRegisterMessage(label, point.index, insn))
        val captureType =
            type?.let(::descriptor)
                ?: registerAType(point.method, point.index, label)
                ?: error(
                    "$label: cannot infer the type of the value at instruction ${point.index} (${insn.opcode}); pass captureAs(\"$name\", type)"
                )
        Capture(name, captureType, register, CaptureOrigin(point.anchor, point.writes(register)))
    }

    /**
     * Captures argument [argument] of this instruction for `capture(name)`. Instructions count the
     * values they take as a call counts its arguments, with a wide argument counted once. A call
     * passes the receiver first, an array write is `set(array, index, value)`, an instance field
     * write `set(object, value)`, a branch `test(value)`, and a return `return(value)`. Arithmetic,
     * conversions and numeric comparisons pass their operands.
     */
    fun captureArgumentAs(name: String, argument: Int, type: String? = null): PointTarget =
        capturing { point ->
            val insns = point.method.instructions
            val origins = ArgumentOrigins(point.method)
            val register = origins.atEntry(insns, point.index, argument, point.anchor.head, label)
            val captureType =
                type?.let(::descriptor)
                    ?: origins.operandTypes(insns, point.index)?.getOrNull(argument)
                    ?: error(
                        "$label: cannot infer the type of argument $argument at instruction ${point.index} (${insns[point.index].opcode}); pass captureArgumentAs(\"$name\", $argument, type)"
                    )
            Capture(name, captureType, register, CaptureOrigin(point.anchor, written = false))
        }

    /**
     * Finds the unique instruction defining argument [argument] of this instruction, including
     * exception paths. Register-to-register moves are followed to the instruction that produced the
     * value. Fails for multiple reaching definitions or an incoming parameter. Arguments are
     * counted as in [captureArgumentAs].
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
                    "$label: ${insns[point.index].opcode} has no argument $argument"
                }
            val register = insns[point.index].operands(label)!![argument].register
            val index =
                origins.sites.singleOrNull()?.takeUnless { origins.entry || origins.unknown }
                    ?: error(writerMessage(label, point.method, point.index, register, origins))
            point.at(runtime, label, index).resolution(label)
        }
    }

    /** The one branch or switch that jumps to this instruction. */
    fun jumpSource(debugName: String? = null): PointTarget {
        val base = this
        val label = debugName ?: "${base.label}.jumpSource"
        return PointTarget(label, method) { runtime ->
            val point = runtime.resolve(base).value
            val sources =
                requireNotNull(jumpSources(point.method.handle, point.index.toUInt())) {
                    "$label: the control flow of ${point.method.descriptor} cannot be followed"
                }
            val index =
                sources.singleOrNull()?.toInt()
                    ?: error(
                        "$label: ${sources.size} instructions jump to instruction ${point.index} of ${point.method.descriptor}, not one: ${sources.joinToString()}"
                    )
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

/** Whether the point's instruction writes [register], so the next instruction sees its value. */
private fun ResolvedPoint.writes(register: Int): Boolean {
    val next = method.jumps(index)?.fallsThrough ?: return false
    return method.writers(next, register)?.indices?.contains(index) == true
}

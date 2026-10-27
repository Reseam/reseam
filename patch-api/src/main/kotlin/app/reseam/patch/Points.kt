// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.points.PointMatchSpec
import app.reseam.patch.points.findSequence
import app.reseam.patch.run.InstructionAnchor
import app.reseam.patch.run.Resolution
import app.reseam.patch.types.Instruction

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
        ResolvedPoint(method, anchor, emptyList()).resolution(label)
    }
}

/** Scan this method once and anchor every match; sequences select their last instruction. */
fun MethodTarget.points(debugName: String? = null, block: PointMatch.() -> Unit): PointsTarget =
    pointsIn(debugName ?: "$label.points", { listOf(this) }, block)

/** Scan only the selected methods, once each, and anchor all results before returning. */
fun MethodsTarget.points(debugName: String? = null, block: PointMatch.() -> Unit): PointsTarget =
    pointsIn(debugName ?: "$label.points", { all }, block)

/** A deferred selection of instruction points, anchored together before any edits. */
class PointsTarget
internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<List<PointTarget>>,
) : Target<List<PointTarget>>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<List<PointTarget>> = resolver(runtime)

    val all: List<PointTarget>
        get() = resolved

    /** Runs [block] with each matching point as its receiver. */
    fun forEach(block: PointTarget.() -> Unit) = all.forEach(block)

    /** Require exactly one point, when used, with all matching locations in the diagnostic. */
    fun single(): PointTarget {
        val selection = this
        val owner =
            MethodTarget("$label.method") { runtime ->
                val winner = selection.only(runtime)
                runtime.resolve(winner.method)
            }
        return PointTarget(label, owner) { runtime -> runtime.resolve(selection.only(runtime)) }
    }

    private fun only(runtime: PatchRuntime): PointTarget {
        val selection = runtime.resolve(this)
        return selection.value.singleOrNull()
            ?: error(
                "$label: expected one point, got ${selection.value.size}: " +
                    selection.value.joinToString { "${it.method.descriptor}[${it.index}]" } +
                    "\n" +
                    selection.report.reasons.joinToString("; ")
            )
    }
}

private fun pointsIn(
    label: String,
    methods: () -> List<MethodTarget>,
    block: PointMatch.() -> Unit,
): PointsTarget =
    PointsTarget(label) { runtime ->
        val diagnostics = mutableListOf<String>()
        var considered = 0
        val points =
            methods()
                .distinctBy { it.method.handle }
                .flatMap { owner ->
                    val method = runtime.resolve(owner).value
                    val insns = method.instructions
                    val spec = PointMatchSpec(method).apply(block)
                    val steps = spec.steps()
                    val hits = linkedSetOf<Int>()
                    var start = 0
                    while (start < insns.size) {
                        val match = findSequence(insns, steps, start) ?: break
                        hits += match.last()
                        start = match.first() + 1
                    }
                    considered += insns.size
                    diagnostics += spec.diagnostics
                    hits.map { index ->
                        val point =
                            ResolvedPoint(
                                method,
                                runtime.edits.anchor(method, index, label),
                                emptyList(),
                            )
                        PointTarget(label, owner) {
                            point.resolution(label)
                        }
                    }
                }
        Resolution(
            points,
            MatchReport(
                label,
                "${points.size} points",
                considered,
                listOf("scanned selected methods") + diagnostics,
                emptyList(),
            ),
        )
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

    internal val instruction: Instruction
        get() = method.instructions[index]

    internal fun withCaptures(captures: List<Capture>) = ResolvedPoint(method, anchor, captures)

    internal fun resolution(label: String) =
        Resolution.direct(this, label, "${method.descriptor}[$index]")

    internal fun at(runtime: PatchRuntime, label: String, index: Int) =
        ResolvedPoint(method, runtime.edits.anchor(method, index, label), captures)
}

class Capture internal constructor(val name: String, val type: String, register: Int) {
    /** Growing a method's frame renumbers its registers, so a capture follows its own. */
    var register: Int = register
        private set

    internal fun relocate(base: Int, additional: Int) {
        if (register >= base) register += additional
    }
}

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

/** A deferred selection of instruction points, anchored together before any edits. */
class PointsTarget
internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<List<PointTarget>>,
) : Target<List<PointTarget>>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<List<PointTarget>> = resolver(runtime)

    val all: List<PointTarget>
        get() = resolved

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

/** Scan this method once and anchor every match; sequences select their last instruction. */
fun MethodTarget.points(debugName: String? = null, block: PointMatch.() -> Unit): PointsTarget =
    pointsIn(debugName ?: "$label.points", { listOf(this) }, block)

/** Scan only the selected methods, once each, and anchor all results before returning. */
fun MethodsTarget.points(debugName: String? = null, block: PointMatch.() -> Unit): PointsTarget =
    pointsIn(debugName ?: "$label.points", { all }, block)

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
                            Resolution(point, wrapped(label, "${method.descriptor}[$index]"))
                        }
                    }
                }
        Resolution(
            points,
            SearchMatchReport(
                label,
                "${points.size} points",
                considered,
                listOf("scanned selected methods") + diagnostics,
                emptyList(),
            ),
        )
    }

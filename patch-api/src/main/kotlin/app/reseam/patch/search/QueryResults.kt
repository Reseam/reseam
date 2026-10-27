// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.MatchReport
import app.reseam.patch.dex.Method
import app.reseam.patch.proto
import app.reseam.patch.types.MethodRef

internal class Ranker<S>(val label: String, val block: S.() -> Int)

internal class Scored<T>(val value: T, val ranks: List<Pair<String, Int>>) {
    val score: Int = ranks.sumOf { it.second }
}

internal class Rejected<T>(val value: T, val failure: String)

private const val MAX_REJECTED = 32

internal fun <T> MutableList<Rejected<T>>.addBounded(
    candidate: Rejected<T>,
    describe: (Rejected<T>) -> String,
) {
    add(candidate)
    if (size <= MAX_REJECTED) return
    sortBy(describe)
    subList(MAX_REJECTED, size).clear()
}

internal class Evaluated<T>(
    val accepted: List<Scored<T>>,
    val considered: Int,
    val pipeline: List<String>,
    val exhaustedBy: String?,
    val rejected: List<String>,
    val describe: (T) -> String,
    private val reasons: (Scored<T>) -> List<String>,
) {
    fun report(debugName: String?, winner: Scored<T>): MatchReport =
        MatchReport(
            debugName ?: "anonymous",
            describe(winner.value),
            considered,
            reasons(winner),
            accepted
                .filter { it !== winner }
                .take(3)
                .map { "${describe(it.value)} [score=${it.score}]" },
        )

    fun noMatchReport(debugName: String?): MatchReport =
        MatchReport(
            debugName ?: "anonymous",
            "<no match>",
            considered,
            buildList {
                addAll(pipeline)
                exhaustedBy?.let { add("candidate intersection exhausted at $it") }
                if (rejected.isNotEmpty()) add("no candidate satisfied the full structural query")
            },
            rejected,
        )
}

internal data class MethodSignature(val owner: String, val name: String, val proto: String)

internal val MethodRef.signature: MethodSignature
    get() = MethodSignature(definingClass, name, proto)

internal val Method.signature: MethodSignature
    get() = MethodSignature(owner, name, proto)

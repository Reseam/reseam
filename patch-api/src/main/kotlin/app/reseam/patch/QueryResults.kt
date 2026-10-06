// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes

internal class Ranker<S>(val label: String, val block: S.() -> Int)

internal class Scored<T>(val value: T, val score: Int, val reasons: List<String>)

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
) {
    fun report(debugName: String?, winner: Scored<T>): MatchReport =
        SearchMatchReport(
            debugName ?: "anonymous",
            describe(winner.value),
            considered,
            winner.reasons,
            accepted
                .filter { it !== winner }
                .take(3)
                .map { "${describe(it.value)} [score=${it.score}]" },
        )

    fun noMatchReport(debugName: String?): MatchReport =
        SearchMatchReport(
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

internal open class RankScopeImpl(protected val index: SearchIndex, override val type: String) :
    RankScope {
    override fun methods(proto: String): List<Method> =
        index.classFor(type)?.methods?.filter { it.proto == proto }.orEmpty()

    override fun zeroArgListGetters(): Int =
        index.classFor(type)?.methods?.count { it.proto == "()${Type.List}" } ?: 0
}

internal class MethodRankScopeImpl(index: SearchIndex, override val method: Method) :
    RankScopeImpl(index, method.owner), MethodRankScope {
    override val paramCount: Int
        get() = method.parameterTypes.size

    override fun callSitesFollowedByCast(type: String, lookAhead: Int): Int =
        index.followedByCheckCast(method, descriptor(type), lookAhead)
}

internal class ClassRankScopeImpl(index: SearchIndex, override val classDef: DexClass) :
    RankScopeImpl(index, classDef.descriptor), ClassRankScope

internal class CandidatePool<T>(
    val candidates: List<T>,
    val pipeline: List<String>,
    val considered: Int,
    val nearMissSeed: List<T>,
    val exhaustedBy: String?,
)

internal data class MethodSignature(val owner: String, val name: String, val proto: String)

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.MatchReport
import app.reseam.patch.PatchRuntime
import app.reseam.patch.RankScope
import app.reseam.patch.dex.Opcode
import app.reseam.patch.run.Resolution

internal enum class QueryMultiplicity {
    UNIQUE,
    FIRST,
}

internal enum class SearchDomain {
    APP,
    INCLUDE_EXTENSIONS,
}

internal abstract class QuerySpec<T : Any, S : RankScope>(private val kind: String) {
    private val rankers = mutableListOf<Ranker<S>>()
    private var multiplicity = QueryMultiplicity.UNIQUE
    private var domain = SearchDomain.APP

    fun rankBy(label: String, block: S.() -> Int) {
        rankers += Ranker(label, block)
    }

    fun first() {
        multiplicity = QueryMultiplicity.FIRST
    }

    fun includeExtensions() {
        domain = SearchDomain.INCLUDE_EXTENSIONS
    }

    /** Excludes extensions by default so lazy linking cannot change query results. */
    fun isHiddenExtension(dexIndex: Int, index: SearchIndex): Boolean =
        domain == SearchDomain.APP && index.isExtensionDex(dexIndex)

    fun extensionReason(): String? =
        if (domain == SearchDomain.INCLUDE_EXTENSIONS) "including extension classes" else null

    protected abstract fun plan(runtime: PatchRuntime): QueryPlan<T>

    protected open fun prefetch(runtime: PatchRuntime, candidates: List<T>) {}

    protected abstract fun mismatch(value: T, runtime: PatchRuntime): String?

    protected abstract fun matchReasons(): List<String>

    protected abstract fun rankScope(index: SearchIndex, value: T): S

    protected abstract fun describe(value: T): String

    fun resolveOne(runtime: PatchRuntime, debugName: String?): Resolution<T> {
        val plan = plan(runtime)
        val evaluated = evaluate(runtime, plan.narrowedPool())
        val winner =
            evaluated.accepted.firstOrNull()
                ?: error(
                    noMatchMessage(
                        kind,
                        diagnose(runtime, plan).noMatchReport(debugName),
                    )
                )
        val runnerUp = evaluated.accepted.getOrNull(1)
        check(
            multiplicity == QueryMultiplicity.FIRST ||
                runnerUp == null ||
                rankers.isNotEmpty() && winner.score > runnerUp.score
        ) {
            "${evaluated.accepted.size} ${kind}s matched '${debugName ?: "anonymous"}'; add constraints, rank them, or take first(): " +
                evaluated.accepted.take(6).joinToString("; ") {
                    "${describe(it.value)} [score=${it.score}]"
                }
        }
        return Resolution(winner.value, evaluated.report(debugName, winner))
    }

    fun resolveAll(runtime: PatchRuntime, debugName: String?): Resolution<List<T>> {
        val evaluated = evaluate(runtime, plan(runtime).narrowedPool())
        val report =
            MatchReport(
                debugName ?: "anonymous",
                "${evaluated.accepted.size} $kind(s)",
                evaluated.considered,
                evaluated.pipeline,
                evaluated.accepted.take(3).map { describe(it.value) },
            )
        return Resolution(evaluated.accepted.map { it.value }, report)
    }

    /**
     * A failed evaluation redone over every seed, keeping the candidates it rejected, so its report
     * names what emptied the query and what came closest.
     */
    private fun diagnose(runtime: PatchRuntime, plan: QueryPlan<T>) =
        evaluate(runtime, plan.exhaustivePool(), mutableListOf())

    /** Collects rejected candidates into [rejected] when given; only failure reports read them. */
    private fun evaluate(
        runtime: PatchRuntime,
        pool: CandidatePool<T>,
        rejected: MutableList<Rejected<T>>? = null,
    ): Evaluated<T> {
        val scored = mutableListOf<Scored<T>>()
        for (batch in pool.candidates.ifEmpty { pool.nearMissSeed }.chunked(CANDIDATE_BATCH)) {
            prefetch(runtime, batch)
            for (value in batch) {
                val failure = mismatch(value, runtime)
                if (failure != null) {
                    rejected?.addBounded(Rejected(value, failure)) { describe(it.value) }
                    continue
                }
                val scope = rankScope(runtime.index, value)
                scored += Scored(value, rankers.map { it.label to it.block(scope) })
            }
        }
        return Evaluated(
            accepted = ranked(scored),
            considered = pool.considered,
            pipeline = pool.pipeline,
            exhaustedBy = pool.exhaustedBy,
            rejected =
                rejected
                    .orEmpty()
                    .sortedBy { describe(it.value) }
                    .take(3)
                    .map { "${describe(it.value)} [missed: ${it.failure}]" },
            describe = ::describe,
            reasons = {
                pool.pipeline + matchReasons() + it.ranks.map { (label, score) -> "$label=$score" }
            },
        )
    }

    /** Best score first, ties in description order; each description is built once. */
    private fun ranked(scored: List<Scored<T>>): List<Scored<T>> =
        if (scored.size < 2) scored
        else
            scored
                .map { it to describe(it.value) }
                .sortedWith(
                    compareByDescending<Pair<Scored<T>, String>> { it.first.score }
                        .thenBy { it.second }
                )
                .map { it.first }
}

internal fun noMatchMessage(kind: String, report: MatchReport): String = buildString {
    append("No $kind matched '${report.name}'. Searched ${report.considered} candidate(s).")
    if (report.reasons.isNotEmpty()) append("\nReasons: ${report.reasons.joinToString("; ")}")
    if (report.nearMisses.isNotEmpty())
        append("\nNear misses: ${report.nearMisses.joinToString("; ")}")
}

internal fun List<Opcode?>.toPattern() = map { it?.value ?: -1 }.toIntArray()

internal fun describeSequence(opcodes: List<Opcode?>) = opcodes.joinToString {
    it?.toString() ?: "any"
}

internal fun quoted(value: String) = "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

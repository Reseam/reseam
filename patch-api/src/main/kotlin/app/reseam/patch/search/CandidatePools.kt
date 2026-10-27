// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

internal class CandidatePool<T>(
    val candidates: List<T>,
    val pipeline: List<String>,
    val considered: Int,
    val nearMissSeed: List<T>,
    val exhaustedBy: String?,
)

/**
 * Candidates are verified in batches of this size. Once seeds narrow a query to one batch, checking
 * each candidate costs less than computing another seed, which scans the whole APK.
 */
internal const val CANDIDATE_BATCH = 256

/** The handles one constraint admits, with the label match reports show for it. */
internal class Seeded(val label: String, val handles: Set<UInt>)

/**
 * A query's seeds, cheapest to compute first. Each runs only when a pool reaches it, and the query
 * verifies every constraint per candidate, so any subset of seeds finds the same matches.
 */
internal class QueryPlan<T>(
    val seeds: List<() -> Seeded>,
    val unit: String,
    val wrap: (Collection<UInt>) -> List<T>,
    val unconstrained: () -> CandidatePool<T>,
) {
    /** Candidates from the plan's seeds in order, until no more than one batch remains. */
    fun narrowedPool(): CandidatePool<T> {
        if (seeds.isEmpty()) return unconstrained()
        var current: Set<UInt>? = null
        val pipeline = mutableListOf<String>()
        for (seed in seeds) {
            val seeded = seed()
            pipeline += "${seeded.label}: ${seeded.handles.size} candidate $unit"
            val next = current?.intersect(seeded.handles) ?: seeded.handles
            current = next
            if (next.size <= CANDIDATE_BATCH) break
        }
        val winners = current.orEmpty()
        return CandidatePool(wrap(winners), pipeline, winners.size, emptyList(), null)
    }

    /** Candidates from every seed, so a query that matched nothing reports what emptied them. */
    fun exhaustivePool(): CandidatePool<T> {
        if (seeds.isEmpty()) return unconstrained()
        var current: Set<UInt>? = null
        var lastNonEmpty: Set<UInt>? = null
        var exhaustedBy: String? = null
        val pipeline = mutableListOf<String>()
        for (seeded in seeds.map { it() }.sortedBy { it.handles.size }) {
            val next = current?.intersect(seeded.handles) ?: seeded.handles
            pipeline += "${seeded.label}: ${seeded.handles.size} candidate $unit"
            if (next.isEmpty()) {
                exhaustedBy = seeded.label
                break
            }
            current = next
            lastNonEmpty = next
        }
        val winners = current.orEmpty()
        val nearMiss = winners.ifEmpty { lastNonEmpty.orEmpty() }
        val considered =
            when {
                winners.isNotEmpty() -> winners.size
                exhaustedBy != null && lastNonEmpty == null -> 0
                else -> nearMiss.size
            }
        return CandidatePool(
            wrap(winners),
            pipeline,
            considered,
            wrap(nearMiss),
            exhaustedBy,
        )
    }
}

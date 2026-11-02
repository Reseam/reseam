// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.inspect

import app.reseam.patch.MatchReport

/**
 * What a target matched in one APK: every candidate that passed its query, the one resolution
 * picks, and, when nothing passed, the candidates that came closest.
 */
internal class TargetInspection(
    val matches: List<String>,
    /** Null when nothing matched, or when a query for one method or class matched several. */
    val winner: String?,
    /** Whether the target resolves to every match, as `methods { }` does, rather than to one. */
    val many: Boolean,
    val considered: Int,
    val reasons: List<String>,
    val nearMisses: List<String>,
    /** Why resolving failed: a target that is not a query, or a query the patch API rejected. */
    val failure: String? = null,
) {
    /** The same inspection, failed with [error]. */
    fun withFailure(error: Exception) =
        TargetInspection(
            matches,
            winner,
            many,
            considered,
            reasons,
            nearMisses,
            error.message ?: error.toString(),
        )

    companion object {
        fun of(report: MatchReport, matches: List<String>, many: Boolean): TargetInspection =
            TargetInspection(
                matches = matches,
                winner = matches.singleOrNull().takeUnless { many },
                many = many,
                considered = report.considered,
                reasons = report.reasons,
                nearMisses = report.nearMisses,
            )

        /** A target that failed before any candidate was looked at. */
        fun failed(error: Exception) =
            TargetInspection(emptyList(), null, false, 0, emptyList(), emptyList())
                .withFailure(error)
    }
}

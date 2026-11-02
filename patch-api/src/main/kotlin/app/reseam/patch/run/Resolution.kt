// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

import app.reseam.patch.MatchReport

/** A target's value and why it was chosen; [searched] is false for a value named outright. */
internal class Resolution<R : Any>(
    val value: R,
    val report: MatchReport,
    val searched: Boolean = true,
) {
    companion object {
        /** A value named outright rather than searched for. */
        fun <R : Any> direct(value: R, name: String, winner: String) =
            Resolution(
                value,
                MatchReport(name, winner, 1, listOf("resolved directly"), emptyList()),
                searched = false,
            )
    }
}

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

import app.reseam.patch.MatchReport
import app.reseam.patch.SearchMatchReport

internal class Resolution<R : Any>(val value: R, val report: MatchReport) {
    companion object {
        /** A value named outright rather than searched for. */
        fun <R : Any> direct(value: R, name: String, winner: String) =
            Resolution(
                value,
                SearchMatchReport(name, winner, 1, listOf("resolved directly"), emptyList()),
            )
    }
}

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.native.growLocalRegisters
import app.reseam.patch.run.edits

internal fun Method.growLocals(additionalLocals: Int) {
    require(additionalLocals in 0..UShort.MAX_VALUE.toInt()) {
        "Invalid local register growth: $additionalLocals"
    }
    val held = edits?.held(this) ?: UShortArray(0)
    edits?.grew(handle, growLocalRegisters(handle, additionalLocals.toUShort(), held))
}

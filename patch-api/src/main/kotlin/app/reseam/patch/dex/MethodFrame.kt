// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.native.findContiguousFreeRegisters
import app.reseam.patch.native.growLocalRegisters
import app.reseam.patch.run.EditKind
import app.reseam.patch.run.edits

internal fun Method.growLocals(additionalLocals: Int): List<Int> {
    require(additionalLocals in 0..UShort.MAX_VALUE.toInt()) {
        "Invalid local register growth: $additionalLocals"
    }
    val base = registersSize - insSize
    // A reserved local is dead between the block that writes it and the one that reads it
    // until the reader is emitted, so liveness alone would let growth stage operands through
    // it.
    val held =
        edits?.reserved(this).orEmpty().flatMap { it.register until it.register + it.wordCount }
    val mapping =
        growLocalRegisters(
            handle,
            additionalLocals.toUShort(),
            UShortArray(held.size) { held[it].toUShort() },
        )
    edits?.relocated(handle, mapping, EditKind.GROW)
    edits?.grewRegisters(handle, base, mapping.registerShift.toInt())
    return mapping.starts.map { it.toInt() }
}

internal fun Method.contiguousFreeRegisters(
    atIndex: Int,
    count: Int,
    exclude: List<Int> = emptyList(),
): List<Int> =
    findContiguousFreeRegisters(
            handle,
            atIndex.toUInt(),
            count.toUInt(),
            UShortArray(exclude.size) { exclude[it].toUShort() },
        )
        .map { it.toInt() }

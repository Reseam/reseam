// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.Capture
import app.reseam.patch.CodeScope
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.growLocals

internal enum class InsertionBranches {
    KEEP_ORIGINAL_TARGET,
    RUN_INSERTION,
}

internal fun Method.insertCode(
    index: Int,
    captures: List<Capture>,
    block: CodeScope.() -> Unit,
    entrySnapshots: MutableMap<Int, EntrySnapshot>? = null,
    returned: Capture? = null,
    branches: InsertionBranches =
        if (entrySnapshots != null) InsertionBranches.RUN_INSERTION
        else InsertionBranches.KEEP_ORIGINAL_TARGET,
) {
    val emitter = CodeEmitter.forInsertion(this, index, captures, returned, entrySnapshots)
    emitter.code.block()
    val instructions = emitter.build()
    val localGrowth = emitter.plannedLocalGrowth
    val insertionIndex =
        if (localGrowth > 0) {
            val indices = growLocals(localGrowth)
            indices[index]
        } else index
    when (branches) {
        InsertionBranches.RUN_INSERTION -> insertOnEveryPath(insertionIndex, instructions)
        InsertionBranches.KEEP_ORIGINAL_TARGET -> insertInstructions(insertionIndex, instructions)
    }
}

internal fun Method.replaceCode(block: CodeScope.() -> Unit) {
    val emitter = CodeEmitter.forReplacement(this)
    emitter.code.block()
    val plan = emitter.buildReplacement()
    replaceBody(plan.registersSize, plan.outsSize, plan.instructions)
}

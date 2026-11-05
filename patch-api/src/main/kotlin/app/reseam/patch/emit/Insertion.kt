// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.Capture
import app.reseam.patch.CodeScope
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.native.emitCode
import app.reseam.patch.run.EditKind
import app.reseam.patch.run.edits
import app.reseam.patch.types.CodeEdit
import app.reseam.patch.types.CodePlacement
import app.reseam.patch.types.CodeStep

/** Writes [block] at [placement]; [captures] are the point values it can read. */
internal fun Method.insertCode(
    placement: CodePlacement,
    block: CodeScope.() -> Unit,
    captures: List<Capture> = emptyList(),
): List<CodeEdit> {
    val emitter = CodeEmitter(this, placement, captures)
    emitter.code.block()
    if (placement == CodePlacement.Body) emitter.requireReturn() else emitter.endInsideBlock()
    val held = if (placement == CodePlacement.Body) null else edits?.held(this)
    val applied = emitCode(handle, placement, emitter.steps, held ?: UShortArray(0))
    for (edit in applied) {
        when (edit) {
            is CodeEdit.Grow -> edits?.grew(handle, edit.field0)
            is CodeEdit.Insert -> edits?.relocated(handle, edit.field0, EditKind.INSERT)
        }
    }
    if (placement == CodePlacement.Body) edits?.bodyReplaced(handle)
    return applied
}

/**
 * Whether a branch targets a label at the end of the block, which resolves to whatever follows it.
 */
private val CodeEmitter.branchesPastEnd: Boolean
    get() {
        val trailingLabels =
            steps
                .takeLastWhile { it is CodeStep.Label }
                .map { (it as CodeStep.Label).field0 }
                .toSet()
        return steps.any { it is CodeStep.Instruction && it.field0.target in trailingLabels }
    }

private fun CodeEmitter.requireReturn() {
    // A branch to a trailing label would target past the end of the body.
    check(steps.lastOrNull { it !is CodeStep.Label }?.fallsThrough == false && !branchesPastEnd) {
        "The replacement for ${info.descriptor} falls off the end of the method; end every path with a return"
    }
}

// Code another hook inserts after this block, such as a later entry hook, would sit between the
// block and the instruction a trailing label resolves to, so a branch there would skip it too.
private fun CodeEmitter.endInsideBlock() {
    if (branchesPastEnd) emit(template { nop() })
}

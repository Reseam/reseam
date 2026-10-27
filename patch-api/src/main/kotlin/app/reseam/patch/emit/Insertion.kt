// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
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
) {
    val emitter = CodeEmitter(this, placement, captures)
    emitter.code.block()
    if (placement == CodePlacement.Body) emitter.requireReturn()
    val held = if (placement == CodePlacement.Body) null else edits?.held(this)
    val applied = emitCode(handle, placement, emitter.steps, held ?: UShortArray(0))
    for (edit in applied) {
        when (edit) {
            is CodeEdit.Grow -> edits?.grew(handle, edit.field0)
            is CodeEdit.Insert -> edits?.relocated(handle, edit.field0, EditKind.INSERT)
        }
    }
    if (placement == CodePlacement.Body) edits?.bodyReplaced(handle)
}

private fun CodeEmitter.requireReturn() {
    // A branch to a trailing label would target past the end of the body.
    val trailingLabels =
        steps.takeLastWhile { it is CodeStep.Label }.map { (it as CodeStep.Label).field0 }.toSet()
    check(
        steps.lastOrNull { it !is CodeStep.Label }?.fallsThrough == false &&
            steps.none { it is CodeStep.Instruction && it.field0.target in trailingLabels }
    ) {
        "The replacement for ${info.descriptor} falls off the end of the method; end every path with a return"
    }
}

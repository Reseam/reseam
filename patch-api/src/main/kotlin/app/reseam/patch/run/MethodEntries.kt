// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

import app.reseam.patch.dex.Method
import app.reseam.patch.types.CodeEdit
import app.reseam.patch.types.MethodEdit

/** Entry hooks follow earlier hooks, while reserved-local initializers stay ahead of them. */
internal class MethodEntries(private val journal: RunJournal) {
    private data class Position(val prologue: Int = 0, val hooks: Int = 0)

    private val positions = HashMap<UInt, Position>()

    fun entry(method: Method): Int = of(method).hooks

    fun prologue(method: Method): Int = of(method).prologue

    fun initialized(method: Method, count: Int) {
        val previous = of(method)
        val prologue = previous.prologue + count
        keep(
            method.handle,
            previous.copy(prologue = prologue, hooks = maxOf(prologue, previous.hooks)),
        )
    }

    fun hooked(method: Method, start: Int, applied: List<CodeEdit>) {
        val end =
            applied.fold(start) { index, edit ->
                when (edit) {
                    is CodeEdit.Grow -> edit.field0.starts[index].toInt()
                    is CodeEdit.Insert -> edit.field0.instructions[index].toInt()
                }
            }
        keep(method.handle, of(method).copy(hooks = end))
    }

    fun relocated(handle: UInt, mapping: MethodEdit) {
        val previous = positions[handle] ?: return
        keep(
            handle,
            Position(
                prologue = mapping.starts[previous.prologue].toInt(),
                hooks = mapping.starts[previous.hooks].toInt(),
            ),
        )
    }

    fun bodyReplaced(handle: UInt) = keep(handle, null)

    private fun of(method: Method): Position =
        positions[method.handle] ?: Position().also { keep(method.handle, it) }

    private fun keep(handle: UInt, position: Position?) {
        val previous = positions[handle]
        if (previous == position) return
        journal.record {
            if (previous == null) positions.remove(handle) else positions[handle] = previous
        }
        if (position == null) positions.remove(handle) else positions[handle] = position
    }
}

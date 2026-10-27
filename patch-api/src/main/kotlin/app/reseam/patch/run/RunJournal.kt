// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

/**
 * Undo actions for state a run keeps across patches, grouped under the engine savepoint that was
 * innermost when each change happened. The engine opens a savepoint before each patch runs and
 * rolls back to one when a patch it ran must leave no trace.
 */
internal class RunJournal {
    private val frames = ArrayList<Pair<Int, MutableList<() -> Unit>>>()

    fun open(savepoint: Int) {
        frames += savepoint to mutableListOf()
    }

    /** Keeps [undo] when a savepoint is open. */
    fun record(undo: () -> Unit) {
        frames.lastOrNull()?.second?.add(undo)
    }

    /** Folds [savepoint]'s undo actions into the savepoint around it, or drops them. */
    fun release(savepoint: Int) {
        val index = frames.indexOfFirst { it.first == savepoint }
        if (index < 0) return
        val undos = frames.removeAt(index).second
        if (index > 0) frames[index - 1].second += undos
    }

    /** Undoes every change since [savepoint] opened, newest first, and closes it and later ones. */
    fun rollback(savepoint: Int) {
        while (frames.lastOrNull()?.let { it.first >= savepoint } == true) {
            frames.removeAt(frames.lastIndex).second.asReversed().forEach { it() }
        }
    }
}

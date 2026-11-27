// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

/**
 * Handles in ascending order, four bytes each. Seeds a run keeps can hold most of the app's
 * methods, which a set of boxed handles would store at about fifteen times the size.
 */
internal class HandleSet private constructor(private val sorted: UIntArray) : AbstractSet<UInt>() {
    override val size: Int
        get() = sorted.size

    override fun contains(element: UInt) = sorted.binarySearch(element) >= 0

    override fun iterator(): Iterator<UInt> = sorted.iterator()

    companion object {
        fun of(handles: UIntArray): HandleSet {
            val sorted = handles.copyOf().apply { sort() }
            val distinct = sorted.indices.count { it == 0 || sorted[it] != sorted[it - 1] }
            if (distinct == sorted.size) return HandleSet(sorted)
            val unique = UIntArray(distinct)
            var next = 0
            sorted.forEachIndexed { i, handle ->
                if (i == 0 || handle != sorted[i - 1]) unique[next++] = handle
            }
            return HandleSet(unique)
        }
    }
}

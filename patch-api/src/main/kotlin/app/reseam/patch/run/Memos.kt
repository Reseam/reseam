// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

/**
 * Results kept until the run's bytecode changes. Every read first brings the memos up to date: a
 * change the engine cannot itemize drops them all, and a method edit drops what each memo says the
 * edited methods made stale.
 */
internal class Memos(private val synchronize: () -> Unit) {
    private val all = mutableListOf<Memo<*, *>>()

    inner class Memo<K, V>(private val onEdit: MutableMap<K, V>.(UIntArray) -> Unit) {
        private val entries = HashMap<K, V>()

        init {
            all += this
        }

        operator fun get(key: K): V? {
            synchronize()
            return entries[key]
        }

        operator fun set(key: K, value: V) {
            entries[key] = value
        }

        fun missing(keys: Collection<K>): List<K> {
            synchronize()
            return keys.filterNot(entries::containsKey)
        }

        fun getOrPut(key: K, compute: () -> V): V {
            synchronize()
            return entries.getOrPut(key, compute)
        }

        fun clear() = entries.clear()

        fun edited(handles: UIntArray) = entries.onEdit(handles)
    }

    /** A memo of what method edits cannot change, such as declarations. */
    fun <K, V> keptOnEdit() = Memo<K, V> {}

    /** A memo of what any method edit may change, such as which methods hold a string. */
    fun <K, V> clearedOnEdit() = Memo<K, V> { clear() }

    /** A memo of each method's own code, which only an edit of that method changes. */
    fun <V> perMethod() = Memo<UInt, V> { it.forEach(::remove) }

    fun clear() = all.forEach { it.clear() }

    fun edited(handles: UIntArray) = all.forEach { it.edited(handles) }
}

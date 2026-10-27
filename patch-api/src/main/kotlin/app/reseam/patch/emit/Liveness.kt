// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

internal fun CodeEmitter.lifetimes(): MutableMap<TemporaryId, Lifetime> {
    fun virtualIds(registers: List<Register>) =
        registers.filterIsInstance<Register.Temporary>().map { it.allocation }.toSet()
    val reads = ops.map { virtualIds(it.reads) }
    val writes = ops.map { virtualIds(it.writes) }
    val labels = ops.mapIndexedNotNull { index, op -> op.label?.let { it to index } }.toMap()
    val successors = ops.mapIndexed { index, op ->
        buildList {
            if (op.fallsThrough && index + 1 < ops.size) add(index + 1)
            op.target?.let { add(labels.getValue(it)) }
        }
    }
    val liveIn = List(ops.size) { mutableSetOf<TemporaryId>() }
    val liveOut = List(ops.size) { mutableSetOf<TemporaryId>() }
    do {
        var changed = false
        for (index in ops.indices.reversed()) {
            val out = successors[index].flatMap { liveIn[it] }.toSet()
            val input = reads[index] + (out - writes[index])
            if (input != liveIn[index] || out != liveOut[index]) {
                liveIn[index].clear()
                liveIn[index] += input
                liveOut[index].clear()
                liveOut[index] += out
                changed = true
            }
        }
    } while (changed)
    val lifetimes = mutableMapOf<TemporaryId, Lifetime>()
    fun touch(ids: Set<TemporaryId>, position: Int) {
        for (id in ids) {
            val previous = lifetimes[id]
            lifetimes[id] =
                Lifetime(
                    minOf(previous?.first ?: position, position),
                    maxOf(previous?.last ?: position, position),
                )
        }
    }
    for (index in ops.indices) {
        touch(liveIn[index] + reads[index], index * 2)
        touch(liveOut[index] + writes[index], index * 2 + 1)
    }
    return lifetimes
}

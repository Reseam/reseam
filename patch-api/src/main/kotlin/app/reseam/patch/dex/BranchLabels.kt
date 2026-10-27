// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.types.Instruction

/** Branches named by label, given code-unit offsets once every instruction is known. */
internal class BranchLabels {
    private val positions = mutableMapOf<String, Int>()
    private val branches = mutableListOf<Branch>()

    private class Branch(
        val index: Int,
        val label: String,
        val encode: (offset: Int) -> Instruction,
    )

    fun mark(label: String, index: Int) {
        positions[label] = index
    }

    fun branch(index: Int, label: String, encode: (offset: Int) -> Instruction) {
        branches += Branch(index, label, encode)
    }

    fun resolve(insns: MutableList<Instruction>) {
        val offsets = insns.runningFold(0) { offset, insn -> offset + insn.codeUnitSize }
        for (branch in branches) {
            val target = positions[branch.label] ?: error("undefined label: '${branch.label}'")
            insns[branch.index] = branch.encode(offsets[target] - offsets[branch.index])
        }
    }
}

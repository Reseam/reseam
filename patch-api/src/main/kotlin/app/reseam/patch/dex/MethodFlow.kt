// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.native.instructionJumps
import app.reseam.patch.native.registerWriters

/**
 * The instructions whose write of a register reaches an instruction, by index into
 * [Method.instructions]. With [fromEntry], the value the method was entered with reaches it too,
 * such as a parameter.
 */
class RegisterWriters internal constructor(val indices: List<Int>, val fromEntry: Boolean)

/**
 * Where an instruction continues, by index into [Method.instructions]: the next instruction when it
 * can fall into it, the target of a branch when taken, and a switch's target for each key.
 */
class Jumps internal constructor(val fallsThrough: Int?, val branch: Int?, val cases: Map<Int, Int>)

/**
 * The writes of [register] that reach instruction [index], following exception paths too. A wide
 * write to the register below also writes this one. Null when control flow cannot be followed.
 */
fun Method.writers(index: Int, register: Int): RegisterWriters? =
    registerWriters(handle, index.toUInt(), register.toUShort())?.let { writers ->
        RegisterWriters(writers.indices.map { it.toInt() }, writers.fromEntry)
    }

/** Where instruction [index] continues. Null when control flow cannot be followed. */
fun Method.jumps(index: Int): Jumps? =
    instructionJumps(handle, index.toUInt())?.let { jumps ->
        Jumps(
            jumps.fallsThrough?.toInt(),
            jumps.branch?.toInt(),
            jumps.caseKeys.indices.associate {
                jumps.caseKeys[it] to jumps.caseTargets[it].toInt()
            },
        )
    }

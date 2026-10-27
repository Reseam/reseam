// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.dex.registerWordCount

internal fun CodeEmitter.registerFits(
    register: Register,
    wordCount: Int,
    constraint: RegisterConstraint,
): Boolean =
    when (register) {
        Register.Void -> false
        is Register.Temporary ->
            tempAllocations.getValue(register.allocation).constraint.maxRegister <=
                constraint.maxRegister
        is Register.Physical ->
            register.index >= 0 &&
                register.index + wordCount - 1 <= constraint.maxRegister &&
                !isShiftedPhysical(register.index)
    }

internal fun CodeEmitter.isShiftedPhysical(register: Int): Boolean =
    mode != EmissionMode.REPLACEMENT && register >= incomingBase

internal fun CodeEmitter.resolveRegister(register: Register): Int =
    when (register) {
        Register.Void -> error("a void value has no register")
        is Register.Temporary -> {
            val allocation = tempAllocations.getValue(register.allocation)
            checkNotNull(allocation.baseRegister) {
                "temporary $register has not been laid out"
            } + register.word
        }
        is Register.Physical ->
            if (isShiftedPhysical(register.index)) register.index + plannedLocalGrowth
            else register.index
    }

internal fun CodeEmitter.incomingValue(offset: Int, type: String): Value {
    if (mode == EmissionMode.REPLACEMENT)
        return entryValues.getOrPut(offset) {
            val id = TemporaryId(nextTempId++)
            tempAllocations[id] =
                TempAllocation(
                    registerWordCount(type),
                    RegisterConstraint.ANY,
                    incomingOffset = offset,
                )
            Value(this, Register.Temporary(id), type)
        }
    val snapshots = entrySnapshots ?: return Value(this, incomingBase + offset, type)
    return entryValues.getOrPut(offset) {
        snapshots[offset]?.let { Value(this, it.register, type) }
            ?: run {
                val id = TemporaryId(nextTempId++)
                tempAllocations[id] =
                    TempAllocation(registerWordCount(type), RegisterConstraint.ANY, offset)
                Value(this, Register.Temporary(id), type)
            }
    }
}

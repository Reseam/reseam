// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.CodeEmitter.Value
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.native.invokeScratchWords
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.InvokeInsn

internal fun CodeEmitter.layoutRegisters() {
    val lifetimes = lifetimes()
    do {
        plannedLocalGrowth = 0
        replacementLocals = MIN_REPLACEMENT_LOCALS
        usedRegisters.clear()
        for (allocation in tempAllocations.values) allocation.baseRegister = null
        for (allocation in tempAllocations.values.filter { it.entryOffset != null }) {
            allocation.baseRegister = allocateLocal(allocation.wordCount, allocation.constraint)
        }
        val savedGrowth = plannedLocalGrowth
        val protected = usedRegisters.toMutableSet()
        for (capture in captures) protected +=
            capture.register until capture.register + registerWordCount(capture.type)
        for (local in reservedLocals) protected +=
            local.register until local.register + local.wordCount
        entrySnapshots?.values?.forEach {
            protected += it.register until it.register + registerWordCount(it.type)
        }
        if (mode != EmissionMode.REPLACEMENT) protected += incomingBase until originalRegistersSize
        for (invoke in invokes) {
            invoke.scratch?.let {
                val position = ops.indexOf(invoke.operation) * 2
                lifetimes[it.allocation] = Lifetime(position, position + 1)
            }
        }
        val active = mutableListOf<Pair<TempAllocation, Int>>()
        val allocations =
            tempAllocations.entries
                .filter {
                    it.value.entryOffset == null &&
                        (it.value.incomingOffset == null || it.value.stageIncoming)
                }
                .sortedWith(
                    compareBy(
                        { lifetimes[it.key]?.first ?: 0 },
                        { it.value.constraint.maxRegister },
                        { it.key.index },
                    )
                )
        for ((id, allocation) in allocations) {
            val usage = lifetimes[id] ?: Lifetime(0, 0)
            val lifetime = if (allocation.incomingOffset != null) Lifetime(0, usage.last) else usage
            active.removeAll { (_, last) -> last < lifetime.first }
            usedRegisters.clear()
            usedRegisters += protected
            for ((slot, _) in active) usedRegisters +=
                checkNotNull(slot.baseRegister) until
                    checkNotNull(slot.baseRegister) + slot.wordCount
            val occupied =
                active
                    .flatMap { (slot, _) ->
                        (checkNotNull(slot.baseRegister) until
                                checkNotNull(slot.baseRegister) + slot.wordCount)
                            .toList()
                    }
                    .toSet()
            allocation.baseRegister = allocateTemp(allocation, savedGrowth, occupied)
            active += allocation to lifetime.last
        }

        for (allocation in
            tempAllocations.values.filter {
                it.incomingOffset != null && !it.stageIncoming
            }) allocation.baseRegister = replacementLocals + allocation.incomingOffset!!

        // Each invoke can add at most one; rerun layout until encodings fit.
        var allocated = false
        for (allocation in
            tempAllocations.values.filter { it.incomingOffset != null && !it.stageIncoming }) {
            if (
                checkNotNull(allocation.baseRegister) + allocation.wordCount - 1 >
                    allocation.constraint.maxRegister
            ) {
                allocation.stageIncoming = true
                allocated = true
            }
        }
        val unscratched = invokes.filter { it.scratch == null }
        val counts =
            invokeScratchWords(
                unscratched.map { invoke ->
                    val registers = invoke.registers.map(::resolveRegister)
                    Instruction.Invoke(
                        InvokeInsn(
                            invoke.opcode.value.toUShort(),
                            UShortArray(registers.size) { registers[it].toUShort() },
                            invoke.ref,
                        )
                    )
                }
            )
        unscratched.forEachIndexed { index, invoke ->
            val words = counts[index].toInt()
            if (words != 0) {
                invoke.scratch = allocTemp(words, RegisterConstraint.ANY)
                allocated = true
            }
        }
    } while (allocated)
    if (mode != EmissionMode.REPLACEMENT)
        for ((offset, value) in entryValues) {
            entrySnapshots!![offset] =
                EntrySnapshot(offset, resolveRegister(value.register), value.type)
        }
}

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

internal fun CodeEmitter.allocateTemp(
    allocation: TempAllocation,
    savedGrowth: Int,
    occupied: Set<Int>,
): Int {
    val words = allocation.wordCount
    fun available(first: Int, end: Int): Int? =
        (first..end - words).firstOrNull { base ->
            base + words - 1 <= allocation.constraint.maxRegister &&
                (base until base + words).none { it in occupied }
        }
    if (mode == EmissionMode.REPLACEMENT) {
        available(0, replacementLocals)?.let {
            return it
        }
        val base = replacementLocals
        val end = base + words
        require(
            end - 1 <= allocation.constraint.maxRegister &&
                end + incomingWords <= UShort.MAX_VALUE.toInt()
        ) {
            "Cannot allocate $words ${allocation.constraint.name.lowercase()} registers in ${info.descriptor}"
        }
        replacementLocals = end
        return base
    }
    val registers =
        method.findContiguousFreeRegisters(insertIndex ?: 0, words, usedRegisters.toList())
    if (registers.size == words && registers.last() <= allocation.constraint.maxRegister)
        return registers.first()
    available(incomingBase + savedGrowth, incomingBase + plannedLocalGrowth)?.let {
        return it
    }
    return allocateLocal(words, allocation.constraint)
}

internal fun CodeEmitter.allocateLocal(wordCount: Int, constraint: RegisterConstraint): Int {
    val index = insertIndex ?: 0
    val grownBase = incomingBase + plannedLocalGrowth
    val grownLast = grownBase + wordCount - 1
    val newRegistersSize = originalRegistersSize + plannedLocalGrowth + wordCount
    require(grownLast <= constraint.maxRegister && newRegistersSize <= UShort.MAX_VALUE.toInt()) {
        "Cannot allocate $wordCount ${constraint.name.lowercase()} scratch register(s) at ${info.descriptor}[$index]; " +
            "plannedLocalGrowth=$plannedLocalGrowth, registersSize=$originalRegistersSize, insSize=${incomingWords}"
    }
    plannedLocalGrowth += wordCount
    usedRegisters += (grownBase..grownLast)
    return grownBase
}

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
            Value(Register.Temporary(id), type)
        }
    val snapshots = entrySnapshots ?: return Value(incomingBase + offset, type)
    return entryValues.getOrPut(offset) {
        snapshots[offset]?.let { Value(it.register, type) }
            ?: run {
                val id = TemporaryId(nextTempId++)
                tempAllocations[id] =
                    TempAllocation(registerWordCount(type), RegisterConstraint.ANY, offset)
                Value(Register.Temporary(id), type)
            }
    }
}

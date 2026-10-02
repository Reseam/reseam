// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.dex

import app.reseam.patch.ActiveRuntime
import app.reseam.patch.EditKind
import app.reseam.patch.edits
import app.reseam.patch.native.addMethodAnnotation
import app.reseam.patch.native.cloneMethod
import app.reseam.patch.native.ensureOutsSize
import app.reseam.patch.native.findAllIndices
import app.reseam.patch.native.findContiguousFreeRegisters
import app.reseam.patch.native.findFreeRegister
import app.reseam.patch.native.findFreeRegisters
import app.reseam.patch.native.growLocalRegisters
import app.reseam.patch.native.indexOfFirst
import app.reseam.patch.native.indexOfFirstFieldAccess
import app.reseam.patch.native.indexOfFirstLiteral
import app.reseam.patch.native.indexOfFirstLiteralReversed
import app.reseam.patch.native.indexOfFirstMethodCall
import app.reseam.patch.native.indexOfFirstReversed
import app.reseam.patch.native.indexOfFirstString
import app.reseam.patch.native.indexOfOpcodeSequence
import app.reseam.patch.native.insertBeforeInstruction
import app.reseam.patch.native.insertInstructions
import app.reseam.patch.native.instructionFieldRef
import app.reseam.patch.native.instructionMethodRef
import app.reseam.patch.native.instructionRegister
import app.reseam.patch.native.instructionStringRef
import app.reseam.patch.native.instructionTypeRef
import app.reseam.patch.native.instructionWideLiteral
import app.reseam.patch.native.removeInstructions
import app.reseam.patch.native.removeMethod
import app.reseam.patch.native.replaceBody
import app.reseam.patch.native.replaceInstruction
import app.reseam.patch.native.replaceLiterals
import app.reseam.patch.native.replaceMethodCall
import app.reseam.patch.native.replaceStrings
import app.reseam.patch.native.returnEarly
import app.reseam.patch.native.returnEarlyInt
import app.reseam.patch.native.returnEarlyObjectNull
import app.reseam.patch.native.returnEarlyWide
import app.reseam.patch.native.setInstructions
import app.reseam.patch.native.setMethodAccessFlags
import app.reseam.patch.types.AnnotationItem
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodInfo
import app.reseam.patch.types.MethodRef

/** A method in the app's bytecode, identified by an engine handle valid for the running patch. */
@JvmInline
value class Method(val handle: UInt) {
    val info: MethodInfo
        get() = ActiveRuntime.current.methodInfo(handle)

    val classDef: DexClass
        get() =
            ActiveRuntime.current.index.classFor(info.classDescriptor)
                ?: error("class not found: ${info.classDescriptor}")

    val descriptor: String
        get() = info.descriptor

    val name: String
        get() = info.methodName

    val owner: String
        get() = info.classDescriptor

    val proto: String
        get() = info.proto

    val returnType: String
        get() = info.returnType

    val parameterTypes: List<String>
        get() = info.parameterTypes

    val isStatic: Boolean
        get() = info.isStatic

    val instructions: List<Instruction>
        get() = ActiveRuntime.current.instructions(handle)

    val instructionCount: Int
        get() = info.instructionCount.toInt()

    val registersSize: Int
        get() = info.registerCount.toInt()

    val insSize: Int
        get() = info.insSize.toInt()

    val outsSize: Int
        get() = info.outsSize.toInt()

    val dexIndex: Int
        get() = info.dexIndex.toInt()

    fun alwaysReturn() = bodyReplaced { returnEarly(handle) }

    fun alwaysReturn(value: Int) = bodyReplaced { returnEarlyInt(handle, value) }

    fun alwaysReturn(value: Boolean) = bodyReplaced { returnEarlyInt(handle, if (value) 1 else 0) }

    fun alwaysReturn(value: Long) = bodyReplaced { returnEarlyWide(handle, value) }

    fun alwaysReturn(value: String) =
        replaceBody(
            insSize + 1,
            0,
            buildInstructions {
                constString(0, value)
                returnObject(0)
            },
        )

    fun alwaysReturnNull() = bodyReplaced { returnEarlyObjectNull(handle) }

    fun setInstructions(insns: List<Instruction>) = bodyReplaced {
        setInstructions(handle, lowerInvokesForBody(insns))
    }

    fun replaceBody(registersSize: Int, outsSize: Int, insns: List<Instruction>) = bodyReplaced {
        replaceBody(
            handle,
            registersSize.toUShort(),
            outsSize.toUShort(),
            lowerInvokesForBody(insns, registersSize),
        )
    }

    fun insertInstruction(index: Int, insn: Instruction) = insertInstructions(index, listOf(insn))

    fun insertInstructions(index: Int, insns: List<Instruction>) {
        require(index >= 0) { "instruction index must be nonnegative" }
        val mapping = insertInstructions(handle, index.toUInt(), lowerInvokesAt(index, insns))
        edits?.relocated(handle, mapping, EditKind.INSERT)
    }

    fun addInstructions(index: Int, block: InstructionBuilder.() -> Unit) =
        insertInstructions(index, buildInstructions(block))

    fun replaceInstruction(index: Int, insn: Instruction) {
        require(index >= 0) { "instruction index must be nonnegative" }
        val lowered = lowerInvokesAt(index, listOf(insn))
        val mapping = replaceInstruction(handle, index.toUInt(), lowered)
        edits?.relocated(handle, mapping, EditKind.REPLACE)
    }

    fun removeInstruction(index: Int) = removeInstructions(index, 1)

    fun removeInstructions(index: Int, count: Int) {
        require(index >= 0 && count >= 0) { "instruction range must be nonnegative" }
        val mapping = removeInstructions(handle, index.toUInt(), count.toUInt())
        edits?.relocated(handle, mapping, EditKind.REMOVE, index until index + count)
    }

    fun replaceString(old: String, new: String): Boolean =
        replaceStrings(handle, old, new, false) > 0u

    fun replaceAllStrings(old: String, new: String): Int =
        replaceStrings(handle, old, new, true).toInt()

    fun replaceLiteral(old: Long, new: Long): Boolean =
        replaceLiterals(handle, old, new, false) > 0u

    fun replaceAllLiterals(old: Long, new: Long): Int =
        replaceLiterals(handle, old, new, true).toInt()

    fun replaceMethodCall(index: Int, target: MethodRef): Boolean =
        replaceMethodCall(handle, index.toUInt(), target.definingClass, target.name, target.proto)

    fun indexOfFirst(opcode: Opcode, start: Int = 0): Int? =
        indexOfFirst(handle, start.toUInt(), opcode.value.toUShort())?.toInt()

    fun indexOfFirstReversed(opcode: Opcode, start: Int): Int? =
        indexOfFirstReversed(handle, start.toUInt(), opcode.value.toUShort())?.toInt()

    fun indexOfFirstLiteral(literal: Long): Int? = indexOfFirstLiteral(handle, literal)?.toInt()

    fun indexOfFirstLiteralReversed(literal: Long): Int? =
        indexOfFirstLiteralReversed(handle, literal)?.toInt()

    fun containsLiteral(literal: Long): Boolean = indexOfFirstLiteral(literal) != null

    fun indexOfFirstString(value: String): Int? = indexOfFirstString(handle, value)?.toInt()

    fun findAllIndices(opcode: Opcode): List<Int> =
        findAllIndices(handle, opcode.value.toUShort()).map { it.toInt() }

    fun indexOfFirstMethodCall(owner: String, name: String, start: Int = 0): Int? =
        indexOfFirstMethodCall(handle, owner, name, start.toUInt())?.toInt()

    fun indexOfFirstFieldAccess(
        opcode: Opcode,
        fieldType: String? = null,
        owner: String? = null,
        start: Int = 0,
    ): Int? =
        indexOfFirstFieldAccess(handle, opcode.value, fieldType, owner, start.toUInt())?.toInt()

    /**
     * The first index where these opcodes run back to back, `null` matching any one instruction.
     */
    fun indexOfOpcodeSequence(vararg opcodes: Opcode?, start: Int = 0): Int? =
        indexOfOpcodeSequence(handle, opcodes.map { it?.value ?: -1 }.toIntArray(), start.toUInt())
            ?.toInt()

    fun indexOfFirstInstruction(start: Int = 0, predicate: Instruction.() -> Boolean): Int? {
        val insns = instructions
        return (start until insns.size).firstOrNull { predicate(insns[it]) }
    }

    fun indexOfFirstInstructionReversed(
        start: Int? = null,
        predicate: Instruction.() -> Boolean,
    ): Int? {
        val insns = instructions
        return ((start ?: insns.lastIndex) downTo 0).firstOrNull { predicate(insns[it]) }
    }

    fun ensureOutsSize(minOutsSize: Int) = ensureOutsSize(handle, minOutsSize.toUShort())

    /**
     * Reserves at least this many locals; lowering may add registers and change instruction
     * indices.
     */
    fun growLocalRegisters(additionalLocals: Int): Boolean {
        growLocals(additionalLocals)
        return true
    }

    internal fun growLocals(additionalLocals: Int): List<Int> {
        require(additionalLocals in 0..UShort.MAX_VALUE.toInt()) {
            "Invalid local register growth: $additionalLocals"
        }
        val base = registersSize - insSize
        // A reserved local is dead between the block that writes it and the one that reads it
        // until the reader is emitted, so liveness alone would let growth stage operands through
        // it.
        val held =
            edits?.reserved(this).orEmpty().flatMap { it.register until it.register + it.wordCount }
        val mapping =
            growLocalRegisters(
                handle,
                additionalLocals.toUShort(),
                UShortArray(held.size) { held[it].toUShort() },
            )
        edits?.relocated(handle, mapping, EditKind.GROW)
        edits?.grewRegisters(handle, base, mapping.registerShift.toInt())
        return mapping.starts.map { it.toInt() }
    }

    internal fun insertOnEveryPath(index: Int, insns: List<Instruction>) {
        val lowered = lowerInvokesAt(index, insns)
        val mapping = insertBeforeInstruction(handle, index.toUInt(), lowered)
        edits?.relocated(handle, mapping, EditKind.INSERT)
    }

    private inline fun <R> bodyReplaced(edit: () -> R): R {
        val result = edit()
        edits?.bodyReplaced(handle)
        return result
    }

    fun findFreeRegister(atIndex: Int, exclude: List<Int> = emptyList()): Int =
        findFreeRegister(
                handle,
                atIndex.toUInt(),
                UShortArray(exclude.size) { exclude[it].toUShort() },
            )
            ?.toInt() ?: error("No free register at $descriptor[$atIndex]")

    fun findFreeRegisters(atIndex: Int, count: Int, exclude: List<Int> = emptyList()): List<Int> =
        findFreeRegisters(
                handle,
                atIndex.toUInt(),
                count.toUInt(),
                UShortArray(exclude.size) { exclude[it].toUShort() },
            )
            .map { it.toInt() }

    fun findContiguousFreeRegisters(
        atIndex: Int,
        count: Int,
        exclude: List<Int> = emptyList(),
    ): List<Int> =
        findContiguousFreeRegisters(
                handle,
                atIndex.toUInt(),
                count.toUInt(),
                UShortArray(exclude.size) { exclude[it].toUShort() },
            )
            .map { it.toInt() }

    fun registerA(index: Int): Int = instructionRegister(handle, index.toUInt(), 0u).toInt()

    fun registerB(index: Int): Int = instructionRegister(handle, index.toUInt(), 1u).toInt()

    fun registerC(index: Int): Int = instructionRegister(handle, index.toUInt(), 2u).toInt()

    fun registerD(index: Int): Int = instructionRegister(handle, index.toUInt(), 3u).toInt()

    fun wideLiteral(index: Int): Long = instructionWideLiteral(handle, index.toUInt())

    fun stringRef(index: Int): String? = instructionStringRef(handle, index.toUInt())

    fun methodRef(index: Int): MethodRef? = instructionMethodRef(handle, index.toUInt())

    fun fieldRef(index: Int): FieldRef? = instructionFieldRef(handle, index.toUInt())

    fun typeRef(index: Int): String? = instructionTypeRef(handle, index.toUInt())

    fun setAccessFlags(flags: Int) {
        val base = registersSize - insSize
        val mapping = setMethodAccessFlags(handle, flags.toUInt())
        if (mapping != null) {
            edits?.grewRegisters(handle, base, mapping.registerShift.toInt())
            edits?.relocated(handle, mapping, EditKind.GROW)
        }
        if (
            AccessFlags.NATIVE.isSet(flags.toUInt()) || AccessFlags.ABSTRACT.isSet(flags.toUInt())
        ) {
            edits?.bodyReplaced(handle)
        }
    }

    fun clone(newName: String? = null): Method = Method(cloneMethod(handle, newName))

    fun remove() = removeMethod(handle)

    fun addAnnotation(annotation: AnnotationItem) = addMethodAnnotation(handle, annotation)

    private fun lowerInvokesAt(index: Int, insns: List<Instruction>): List<Instruction> {
        val reserved = insns.flatMap { it.referencedRegisters }.toSet()
        return lowerInvokes(insns) { wordCount ->
            findContiguousFreeRegisters(index, wordCount, reserved.toList())
        }
    }

    private fun lowerInvokesForBody(
        insns: List<Instruction>,
        targetRegistersSize: Int = registersSize,
    ): List<Instruction> {
        val reserved = insns.flatMap { it.referencedRegisters }.toSet()
        return lowerInvokes(insns) { wordCount ->
            contiguousUnusedRegisters(targetRegistersSize, reserved, wordCount)
                ?: error("no $wordCount consecutive scratch registers available to lower invoke")
        }
    }
}

/**
 * Rewrites `invoke-kind` instructions the 35c format cannot encode (more than five registers, or a
 * register above v15) into `invoke-kind/range`, moving the arguments into scratch registers when
 * they are not consecutive.
 */
fun lowerInvokes(
    insns: List<Instruction>,
    scratch: (wordCount: Int) -> List<Int>,
): List<Instruction> {
    val counts = app.reseam.patch.native.invokeScratchWords(insns)
    val spans =
        insns.indices.mapNotNull { index ->
            val words = counts[index].toInt()
            if (words == 0) null
            else {
                val registers = scratch(words)
                require(registers.all { it in 0..UShort.MAX_VALUE.toInt() }) {
                    "scratch registers must fit the DEX frame"
                }
                app.reseam.patch.types.ScratchSpan(
                    index.toUInt(),
                    UShortArray(registers.size) { registers[it].toUShort() },
                )
            }
        }
    return app.reseam.patch.native.lowerInstructions(insns, spans)
}

val Opcode.rangeVariant: Opcode?
    get() =
        when (this) {
            Opcode.INVOKE_VIRTUAL -> Opcode.INVOKE_VIRTUAL_RANGE
            Opcode.INVOKE_SUPER -> Opcode.INVOKE_SUPER_RANGE
            Opcode.INVOKE_DIRECT -> Opcode.INVOKE_DIRECT_RANGE
            Opcode.INVOKE_STATIC -> Opcode.INVOKE_STATIC_RANGE
            Opcode.INVOKE_INTERFACE -> Opcode.INVOKE_INTERFACE_RANGE
            else -> null
        }

private fun contiguousUnusedRegisters(
    registerCount: Int,
    excluded: Set<Int>,
    wordCount: Int,
): List<Int>? {
    if (wordCount == 0) return emptyList()
    return (0..registerCount - wordCount)
        .map { start -> (start until start + wordCount).toList() }
        .firstOrNull { candidate -> candidate.none { it in excluded } }
}

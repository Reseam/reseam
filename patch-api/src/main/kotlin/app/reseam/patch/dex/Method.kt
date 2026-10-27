// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.native.addMethodAnnotation
import app.reseam.patch.native.cloneMethod
import app.reseam.patch.native.ensureOutsSize
import app.reseam.patch.native.findAllIndices
import app.reseam.patch.native.findFreeRegister
import app.reseam.patch.native.indexOfFirst
import app.reseam.patch.native.indexOfFirstFieldAccess
import app.reseam.patch.native.indexOfFirstLiteral
import app.reseam.patch.native.indexOfFirstLiteralReversed
import app.reseam.patch.native.indexOfFirstMethodCall
import app.reseam.patch.native.indexOfFirstReversed
import app.reseam.patch.native.indexOfFirstString
import app.reseam.patch.native.indexOfOpcodeSequence
import app.reseam.patch.native.insertInstructions
import app.reseam.patch.native.removeInstructions
import app.reseam.patch.native.removeMethod
import app.reseam.patch.native.replaceBody
import app.reseam.patch.native.replaceInstruction
import app.reseam.patch.native.replaceLiterals
import app.reseam.patch.native.replaceMethodCall
import app.reseam.patch.native.replaceStrings
import app.reseam.patch.native.setMethodAccessFlags
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.run.EditKind
import app.reseam.patch.run.edits
import app.reseam.patch.types.AnnotationItem
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodInfo
import app.reseam.patch.types.MethodRef

/** A method in the app's bytecode, identified by an engine handle valid for the running patch. */
@JvmInline
value class Method(val handle: UInt) {
    val info: MethodInfo
        get() = ActiveRuntime.current.run.cache.methodInfo(handle)

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
        get() = ActiveRuntime.current.run.cache.instructions(handle)

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

    fun replaceBody(registersSize: Int, outsSize: Int, insns: List<Instruction>) {
        replaceBody(handle, registersSize.toUShort(), outsSize.toUShort(), insns)
        edits?.bodyReplaced(handle)
    }

    fun insertInstructions(index: Int, insns: List<Instruction>) {
        require(index >= 0) { "instruction index must be nonnegative" }
        val mapping = insertInstructions(handle, index.toUInt(), insns)
        edits?.relocated(handle, mapping, EditKind.INSERT)
    }

    fun replaceInstruction(index: Int, insn: Instruction) {
        require(index >= 0) { "instruction index must be nonnegative" }
        val mapping = replaceInstruction(handle, index.toUInt(), listOf(insn))
        edits?.relocated(handle, mapping, EditKind.REPLACE)
    }

    fun removeInstructions(index: Int, count: Int) {
        require(index >= 0 && count >= 0) { "instruction range must be nonnegative" }
        val mapping = removeInstructions(handle, index.toUInt(), count.toUInt())
        edits?.relocated(handle, mapping, EditKind.REMOVE, index until index + count)
    }

    fun replaceAllStrings(old: String, new: String): Int = replaceStrings(handle, old, new).toInt()

    fun replaceAllLiterals(old: Long, new: Long): Int = replaceLiterals(handle, old, new).toInt()

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

    fun findFreeRegister(atIndex: Int, exclude: List<Int> = emptyList()): Int =
        findFreeRegister(
                handle,
                atIndex.toUInt(),
                UShortArray(exclude.size) { exclude[it].toUShort() },
            )
            ?.toInt() ?: error("No free register at $descriptor[$atIndex]")

    fun setAccessFlags(flags: Int) {
        val mapping = setMethodAccessFlags(handle, flags.toUInt())
        if (mapping != null) edits?.grew(handle, mapping)
        if (
            AccessFlags.NATIVE.isSet(flags.toUInt()) || AccessFlags.ABSTRACT.isSet(flags.toUInt())
        ) {
            edits?.bodyReplaced(handle)
        }
    }

    fun clone(newName: String? = null): Method = Method(cloneMethod(handle, newName))

    fun remove() = removeMethod(handle)

    fun addAnnotation(annotation: AnnotationItem) = addMethodAnnotation(handle, annotation)
}

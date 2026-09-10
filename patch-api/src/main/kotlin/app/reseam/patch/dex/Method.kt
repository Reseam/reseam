// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.dex

import app.reseam.patch.ActiveRuntime
import app.reseam.patch.AnnotationItem
import app.reseam.patch.FieldRef
import app.reseam.patch.Instruction
import app.reseam.patch.InvokeRangeInsn
import app.reseam.patch.MethodInfo
import app.reseam.patch.MethodRef
import app.reseam.patch.addMethodAnnotation
import app.reseam.patch.cloneMethod
import app.reseam.patch.ensureOutsSize
import app.reseam.patch.findAllIndices
import app.reseam.patch.findClass
import app.reseam.patch.findContiguousFreeRegisters
import app.reseam.patch.findFreeRegister
import app.reseam.patch.findFreeRegisters
import app.reseam.patch.getInstructions
import app.reseam.patch.growLocalRegisters
import app.reseam.patch.indexOfFirst
import app.reseam.patch.indexOfFirstFieldAccess
import app.reseam.patch.indexOfFirstLiteral
import app.reseam.patch.indexOfFirstLiteralReversed
import app.reseam.patch.indexOfFirstMethodCall
import app.reseam.patch.indexOfFirstReversed
import app.reseam.patch.indexOfFirstString
import app.reseam.patch.indexOfOpcodeSequence
import app.reseam.patch.insSize
import app.reseam.patch.insertInstructions
import app.reseam.patch.instructionCount
import app.reseam.patch.instructionFieldRef
import app.reseam.patch.instructionMethodRef
import app.reseam.patch.instructionRegister
import app.reseam.patch.instructionStringRef
import app.reseam.patch.instructionTypeRef
import app.reseam.patch.instructionWideLiteral
import app.reseam.patch.methodDex
import app.reseam.patch.outsSize
import app.reseam.patch.registersSize
import app.reseam.patch.removeInstructions
import app.reseam.patch.removeMethod
import app.reseam.patch.replaceBody
import app.reseam.patch.replaceInstruction
import app.reseam.patch.replaceLiterals
import app.reseam.patch.replaceMethodCall
import app.reseam.patch.replaceStrings
import app.reseam.patch.returnEarly
import app.reseam.patch.returnEarlyInt
import app.reseam.patch.returnEarlyObjectNull
import app.reseam.patch.returnEarlyWide
import app.reseam.patch.setInstructions
import app.reseam.patch.setMethodAccessFlags

/** A method in the app's bytecode, identified by an engine handle valid for the running patch. */
@JvmInline
value class Method(val handle: UInt) {
    val info: MethodInfo
        get() = ActiveRuntime.current.methodInfo(handle)

    val classDef: DexClass
        get() = DexClass(findClass(info.classDescriptor) ?: error("class not found: ${info.classDescriptor}"))

    val descriptor: String get() = info.descriptor
    val name: String get() = info.methodName
    val owner: String get() = info.classDescriptor
    val proto: String get() = info.proto
    val returnType: String get() = info.returnType
    val parameterTypes: List<String> get() = info.parameterTypes
    val isStatic: Boolean get() = info.isStatic

    val instructions: List<Instruction> get() = getInstructions(handle)
    val instructionCount: Int get() = instructionCount(handle).toInt()
    val registersSize: Int get() = registersSize(handle).toInt()
    val insSize: Int get() = insSize(handle).toInt()
    val outsSize: Int get() = outsSize(handle).toInt()
    val dexIndex: Int get() = methodDex(handle).toInt()

    fun alwaysReturn() = returnEarly(handle)
    fun alwaysReturn(value: Int) = returnEarlyInt(handle, value)
    fun alwaysReturn(value: Boolean) = returnEarlyInt(handle, if (value) 1 else 0)
    fun alwaysReturn(value: Long) = returnEarlyWide(handle, value)
    fun alwaysReturn(value: String) = replaceBody(insSize + 1, 0, buildInstructions { constString(0, value); returnObject(0) })
    fun alwaysReturnNull() = returnEarlyObjectNull(handle)

    fun setInstructions(insns: List<Instruction>) = setInstructions(handle, lowerInvokesForBody(insns))

    fun replaceBody(registersSize: Int, outsSize: Int, insns: List<Instruction>) =
        replaceBody(handle, registersSize.toUShort(), outsSize.toUShort(), lowerInvokesForBody(insns, registersSize))

    fun insertInstruction(index: Int, insn: Instruction) = insertInstructions(index, listOf(insn))

    fun insertInstructions(index: Int, insns: List<Instruction>) =
        insertInstructions(handle, index.toUInt(), lowerInvokesAt(index, insns))

    fun addInstructions(index: Int, block: InstructionBuilder.() -> Unit) = insertInstructions(index, buildInstructions(block))

    fun replaceInstruction(index: Int, insn: Instruction) {
        val lowered = lowerInvokesAt(index, listOf(insn))
        if (lowered.size == 1) {
            replaceInstruction(handle, index.toUInt(), lowered.single())
        } else {
            removeInstructions(handle, index.toUInt(), 1u)
            insertInstructions(handle, index.toUInt(), lowered)
        }
    }

    fun removeInstruction(index: Int) = removeInstructions(index, 1)
    fun removeInstructions(index: Int, count: Int) = removeInstructions(handle, index.toUInt(), count.toUInt())

    fun replaceString(old: String, new: String): Boolean = replaceStrings(handle, old, new, false) > 0u
    fun replaceAllStrings(old: String, new: String): Int = replaceStrings(handle, old, new, true).toInt()
    fun replaceLiteral(old: Long, new: Long): Boolean = replaceLiterals(handle, old, new, false) > 0u
    fun replaceAllLiterals(old: Long, new: Long): Int = replaceLiterals(handle, old, new, true).toInt()
    fun replaceMethodCall(index: Int, target: MethodRef): Boolean =
        replaceMethodCall(handle, index.toUInt(), target.definingClass, target.name, target.proto)

    fun indexOfFirst(opcode: Opcode, start: Int = 0): Int? = indexOfFirst(handle, start.toUInt(), opcode.value.toUShort())?.toInt()
    fun indexOfFirstReversed(opcode: Opcode, start: Int): Int? = indexOfFirstReversed(handle, start.toUInt(), opcode.value.toUShort())?.toInt()
    fun indexOfFirstLiteral(literal: Long): Int? = indexOfFirstLiteral(handle, literal)?.toInt()
    fun indexOfFirstLiteralReversed(literal: Long): Int? = indexOfFirstLiteralReversed(handle, literal)?.toInt()
    fun containsLiteral(literal: Long): Boolean = indexOfFirstLiteral(literal) != null
    fun indexOfFirstString(value: String): Int? = indexOfFirstString(handle, value)?.toInt()
    fun findAllIndices(opcode: Opcode): List<Int> = findAllIndices(handle, opcode.value.toUShort()).toList()
    fun indexOfFirstMethodCall(owner: String, name: String, start: Int = 0): Int? =
        indexOfFirstMethodCall(handle, owner, name, start.toUInt())?.toInt()
    fun indexOfFirstFieldAccess(opcode: Opcode, fieldType: String? = null, owner: String? = null, start: Int = 0): Int? =
        indexOfFirstFieldAccess(handle, opcode.value, fieldType, owner, start.toUInt())?.toInt()
    fun indexOfOpcodeSequence(vararg opcodes: Opcode, start: Int = 0): Int? =
        indexOfOpcodeSequence(handle, opcodes.map { it.value }.toIntArray(), start.toUInt())?.toInt()

    fun indexOfFirstInstruction(start: Int = 0, predicate: Instruction.() -> Boolean): Int? {
        val insns = instructions
        return (start until insns.size).firstOrNull { predicate(insns[it]) }
    }

    fun indexOfFirstInstructionReversed(start: Int? = null, predicate: Instruction.() -> Boolean): Int? {
        val insns = instructions
        return ((start ?: insns.lastIndex) downTo 0).firstOrNull { predicate(insns[it]) }
    }

    fun ensureOutsSize(minOutsSize: Int) = ensureOutsSize(handle, minOutsSize.toUShort())
    fun growLocalRegisters(additionalLocals: Int): Boolean = growLocalRegisters(handle, additionalLocals.toUShort())
    fun findFreeRegister(atIndex: Int, exclude: List<Int> = emptyList()): Int =
        findFreeRegister(handle, atIndex.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).toInt()
    fun findFreeRegisters(atIndex: Int, count: Int, exclude: List<Int> = emptyList()): List<Int> =
        findFreeRegisters(handle, atIndex.toUInt(), count.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).map { it.toInt() }
    fun findContiguousFreeRegisters(atIndex: Int, count: Int, exclude: List<Int> = emptyList()): List<Int> =
        findContiguousFreeRegisters(handle, atIndex.toUInt(), count.toUInt(), ShortArray(exclude.size) { exclude[it].toShort() }).map { it.toInt() }

    fun registerA(index: Int): Int = instructionRegister(handle, index.toUInt(), 0u).toInt()
    fun registerB(index: Int): Int = instructionRegister(handle, index.toUInt(), 1u).toInt()
    fun registerC(index: Int): Int = instructionRegister(handle, index.toUInt(), 2u).toInt()
    fun registerD(index: Int): Int = instructionRegister(handle, index.toUInt(), 3u).toInt()
    fun wideLiteral(index: Int): Long = instructionWideLiteral(handle, index.toUInt())
    fun stringRef(index: Int): String? = instructionStringRef(handle, index.toUInt())
    fun methodRef(index: Int): MethodRef? = instructionMethodRef(handle, index.toUInt())
    fun fieldRef(index: Int): FieldRef? = instructionFieldRef(handle, index.toUInt())
    fun typeRef(index: Int): String? = instructionTypeRef(handle, index.toUInt())

    fun setAccessFlags(flags: Int) = setMethodAccessFlags(handle, flags.toUInt())
    fun clone(newName: String? = null): Method = Method(cloneMethod(handle, newName))
    fun remove() = removeMethod(handle)
    fun addAnnotation(annotation: AnnotationItem) = addMethodAnnotation(handle, annotation)

    private fun lowerInvokesAt(index: Int, insns: List<Instruction>): List<Instruction> {
        val reserved = insns.flatMap { it.referencedRegisters }.toSet()
        return lowerInvokes(insns) { wordCount -> findContiguousFreeRegisters(index, wordCount, reserved.toList()) }
    }

    private fun lowerInvokesForBody(insns: List<Instruction>, targetRegistersSize: Int = registersSize): List<Instruction> {
        val reserved = insns.flatMap { it.referencedRegisters }.toSet()
        return lowerInvokes(insns) { wordCount ->
            contiguousUnusedRegisters(targetRegistersSize, reserved, wordCount)
                ?: error("no $wordCount consecutive scratch registers available to lower invoke")
        }
    }
}

/**
 * Rewrites `invoke-kind` instructions the 35c format cannot encode (more than
 * five registers, or a register above v15) into `invoke-kind/range`, moving
 * the arguments into scratch registers when they are not consecutive.
 */
fun lowerInvokes(insns: List<Instruction>, scratch: (wordCount: Int) -> List<Int>): List<Instruction> =
    insns.flatMap { insn -> lowerInvoke(insn, scratch) }

private fun lowerInvoke(insn: Instruction, scratch: (wordCount: Int) -> List<Int>): List<Instruction> {
    val invoke = insn as? Instruction.Invoke ?: return listOf(insn)
    val regs = invoke.value0.registers.map { it.toInt() }
    if (regs.size <= 5 && regs.all { it in 0..15 }) return listOf(insn)

    val opcode = Opcode.of(invoke.value0.opcode.toInt())
    val rangeOpcode = opcode?.rangeVariant ?: error("$opcode does not support invoke/range lowering")
    val method = invoke.value0.method
    if (regs.isConsecutive()) {
        return listOf(Instruction.InvokeRange(InvokeRangeInsn(rangeOpcode.value.toUShort(), (regs.firstOrNull() ?: 0).toUShort(), regs.size.toUShort(), method)))
    }

    val argTypes = buildList {
        if (opcode != Opcode.INVOKE_STATIC) add(method.definingClass)
        addAll(method.parameterTypes)
    }
    val expectedWords = argTypes.sumOf(::registerWordCount)
    require(expectedWords == regs.size) { "invoke ${method.descriptor} uses ${regs.size} register words, expected $expectedWords" }

    val target = scratch(regs.size)
    require(target.size == regs.size && target.isConsecutive()) { "scratch allocator returned an unusable register span: $target" }

    return buildInstructions {
        var src = 0
        for (type in argTypes) {
            if (registerWordCount(type) == 2) {
                require(regs[src + 1] == regs[src] + 1) { "wide argument must occupy consecutive registers, got ${regs[src]} and ${regs[src + 1]}" }
            }
            moveTyped(target[src], regs[src], type)
            src += registerWordCount(type)
        }
        invokeRange(rangeOpcode, method, target.first(), regs.size)
    }
}

val Opcode.rangeVariant: Opcode?
    get() = when (this) {
        Opcode.INVOKE_VIRTUAL -> Opcode.INVOKE_VIRTUAL_RANGE
        Opcode.INVOKE_SUPER -> Opcode.INVOKE_SUPER_RANGE
        Opcode.INVOKE_DIRECT -> Opcode.INVOKE_DIRECT_RANGE
        Opcode.INVOKE_STATIC -> Opcode.INVOKE_STATIC_RANGE
        Opcode.INVOKE_INTERFACE -> Opcode.INVOKE_INTERFACE_RANGE
        else -> null
    }

private fun List<Int>.isConsecutive(): Boolean = indices.all { it == 0 || this[it] == this[it - 1] + 1 }

private fun contiguousUnusedRegisters(registerCount: Int, excluded: Set<Int>, wordCount: Int): List<Int>? {
    if (wordCount == 0) return emptyList()
    return (0..registerCount - wordCount)
        .map { start -> (start until start + wordCount).toList() }
        .firstOrNull { candidate -> candidate.none { it in excluded } }
}

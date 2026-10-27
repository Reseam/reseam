// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.regA
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.stringValue
import app.reseam.patch.dex.typeRef
import app.reseam.patch.points.ArgumentOrigins
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

private const val ENUM = "Ljava/lang/Enum;"

/**
 * The constants an enum's `<clinit>` constructs, by the name each passes to the constructor. A
 * constant is a new instance of the enum, or of its subclass for a constant with a body. Any
 * constructor in that hierarchy may initialize it: optimizers inline an enum's own constructor down
 * to `Enum.<init>(String, int)`, and reorder its parameters, so the name is the argument each
 * constructor forwards to `Enum.<init>`.
 */
internal fun readEnumConstants(classDef: DexClass, index: SearchIndex): Map<String, FieldRef> {
    if (classDef.superclass != ENUM) return emptyMap()
    val type = classDef.descriptor
    val initializer = index.methodsInClass(type, "<clinit>").singleOrNull() ?: return emptyMap()
    val insns = initializer.instructions
    val origins = ArgumentOrigins(initializer)
    fun sites(found: ArgumentOrigins.Origins?) =
        found?.takeUnless { it.unknown || it.entry || it.sites.isEmpty() }?.sites
    fun createsConstant(site: Int): Boolean {
        val created =
            insns[site].takeIf { it.opcode == Opcode.NEW_INSTANCE }?.typeRef ?: return false
        return created == type || index.classFor(created)?.superclass == type
    }

    val nameArguments = mutableMapOf<MethodRef, Int?>()
    val named = mutableMapOf<Set<Int>, String>()
    insns.forEachIndexed { at, insn ->
        val ref = insn.methodRef ?: return@forEachIndexed
        if (insn.opcode !in constructorCalls || ref.name != "<init>") return@forEachIndexed
        val instance =
            sites(origins.argument(insns, at, 0))?.takeIf { it.all(::createsConstant) }
                ?: return@forEachIndexed
        val name =
            nameArguments.getOrPut(ref) { nameArgument(ref, index, mutableSetOf()) }
                ?: return@forEachIndexed
        named[instance] =
            sites(origins.argument(insns, at, name))
                ?.map { insns[it].stringValue ?: return@forEachIndexed }
                ?.distinct()
                ?.singleOrNull() ?: return@forEachIndexed
    }
    return buildMap {
        insns.forEachIndexed { at, insn ->
            val field = insn.fieldRef ?: return@forEachIndexed
            if (
                insn.opcode != Opcode.SPUT_OBJECT ||
                    field.definingClass != type ||
                    field.fieldType != type
            )
                return@forEachIndexed
            val instance = sites(origins.register(insns, at, insn.regA ?: return@forEachIndexed))
            named[instance]?.let { put(it, field) }
        }
    }
}

private fun nameArgument(
    constructor: MethodRef,
    index: SearchIndex,
    seen: MutableSet<MethodRef>,
): Int? {
    if (constructor.definingClass == ENUM) return 1
    if (!seen.add(constructor)) return null
    val method = index.methodFor(constructor) ?: return null
    val insns = method.instructions
    val origins = ArgumentOrigins(method)
    fun parameter(at: Int, argument: Int) =
        origins
            .argument(insns, at, argument, before = 0)
            ?.takeIf { !it.unknown && it.sites.isEmpty() }
            ?.registersAtBoundary
            ?.singleOrNull()
            ?.let { argumentIn(method, it) }
    insns.forEachIndexed { at, insn ->
        val ref = insn.methodRef ?: return@forEachIndexed
        if (insn.opcode !in constructorCalls || ref.name != "<init>") return@forEachIndexed
        if (parameter(at, 0) != 0) return@forEachIndexed
        return parameter(at, nameArgument(ref, index, seen) ?: return null)
    }
    return null
}

private fun argumentIn(method: Method, register: Int): Int? {
    var next = method.registersSize - method.insSize
    (listOf(method.owner) + method.parameterTypes).forEachIndexed { argument, type ->
        if (next == register) return argument
        next += registerWordCount(type)
    }
    return null
}

private val constructorCalls = setOf(Opcode.INVOKE_DIRECT, Opcode.INVOKE_DIRECT_RANGE)

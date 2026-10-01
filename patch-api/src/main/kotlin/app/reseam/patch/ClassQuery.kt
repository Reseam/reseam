// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.descriptor

internal class ClassQuerySpec : QuerySpec<DexClass, ClassRankScope>("class"), ClassQuery {
    private val stringValues = mutableListOf<String>()
    private val instanceFieldTypes = mutableListOf<String>()
    private val superTypes = mutableListOf<String>()
    private val interfaceTypes = mutableListOf<String>()
    private val sourceFiles = mutableListOf<String>()
    private val predicates = mutableListOf<DexClass.() -> Boolean>()

    override fun strings(vararg values: String) {
        stringValues += values
    }

    override fun hasInstanceField(type: String) {
        instanceFieldTypes += descriptor(type)
    }

    override fun extends(type: String) {
        superTypes += descriptor(type)
    }

    override fun implements(type: String) {
        interfaceTypes += descriptor(type)
    }

    override fun sourceFile(name: String) {
        sourceFiles += name
    }

    override fun custom(predicate: DexClass.() -> Boolean) {
        predicates += predicate
    }

    override fun candidates(runtime: PatchRuntime): CandidatePool<DexClass> {
        val index = runtime.index
        val seeds = mutableListOf<Pair<String, Set<UInt>>>()
        stringValues.forEach {
            seeds +=
                "strings(${quoted(it)})" to
                    index.classesWithString(it).map(DexClass::handle).toSet()
        }
        sourceFiles.forEach {
            seeds +=
                "sourceFile(${quoted(it)})" to
                    index.classesWithSourceFile(it).map(DexClass::handle).toSet()
        }
        if (seeds.isEmpty()) {
            instanceFieldTypes.forEach {
                seeds +=
                    "hasInstanceField($it)" to
                        index.classesWithInstanceFieldType(it).map(DexClass::handle).toSet()
            }
        }
        if (seeds.isEmpty()) {
            return CandidatePool(
                index.allClasses,
                listOf(
                    "no selective constraints; considering all ${index.allClasses.size} classes"
                ),
                index.allClasses.size,
                index.allClasses.take(12),
                null,
            )
        }
        return index.poolFromSeeds(seeds, "class(es)") { handles -> handles.map(::DexClass) }
    }

    override fun rankScope(index: SearchIndex, value: DexClass) = ClassRankScopeImpl(index, value)

    override fun describe(value: DexClass) = value.descriptor

    override fun mismatch(value: DexClass, runtime: PatchRuntime): String? {
        val index = runtime.index
        if (isHiddenExtension(value.info.dexIndex.toInt(), index))
            return "defined by a bundle extension"
        if (stringValues.any { !index.classHasString(value, it) }) return "missing required string"
        if (instanceFieldTypes.any { type -> value.instanceFields.none { it.fieldType == type } })
            return "missing instance field"
        if (superTypes.any { !index.classExtends(value, it) }) return "superclass mismatch"
        if (interfaceTypes.any { it !in value.interfaces }) return "interface mismatch"
        if (sourceFiles.any { it != value.sourceFile }) return "source file mismatch"
        if (predicates.any { !it(value) }) return "custom predicate rejected it"
        return null
    }

    override fun matchReasons(): List<String> = buildList {
        if (stringValues.isNotEmpty()) add("matched class strings ${stringValues.joinToString()}")
        if (instanceFieldTypes.isNotEmpty())
            add("matched instance fields ${instanceFieldTypes.joinToString()}")
        if (superTypes.isNotEmpty()) add("matched superclass ${superTypes.joinToString()}")
        if (interfaceTypes.isNotEmpty()) add("matched interfaces ${interfaceTypes.joinToString()}")
        if (sourceFiles.isNotEmpty()) add("matched source file ${sourceFiles.joinToString()}")
        if (predicates.isNotEmpty()) add("matched ${predicates.size} custom predicate(s)")
        extensionReason()?.let(::add)
    }
}

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.ClassQuery
import app.reseam.patch.ClassRankScope
import app.reseam.patch.PatchRuntime
import app.reseam.patch.descriptor
import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.descriptor

internal class ClassQuerySpec : QuerySpec<DexClass, ClassRankScope>("class"), ClassQuery {
    private val stringValues = mutableListOf<String>()
    private val instanceFieldTypes = mutableListOf<String>()
    private val enumNames = mutableListOf<String>()
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

    override fun enumConstants(vararg names: String) {
        enumNames += names
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

    override fun plan(runtime: PatchRuntime): QueryPlan<DexClass> {
        val index = runtime.index
        val handles = { classes: Collection<DexClass> -> classes.map(DexClass::handle).toSet() }
        val selective =
            buildList<() -> Seeded> {
                stringValues.forEach {
                    add { Seeded("strings(${quoted(it)})", handles(index.classesWithString(it))) }
                }
                enumNames.forEach {
                    add {
                        Seeded("enumConstants(${quoted(it)})", handles(index.classesWithString(it)))
                    }
                }
                sourceFiles.forEach {
                    add {
                        Seeded(
                            "sourceFile(${quoted(it)})",
                            handles(index.classesWithSourceFile(it)),
                        )
                    }
                }
            }
        val seeds = selective.ifEmpty {
            buildList {
                interfaceTypes.forEach {
                    add { Seeded("implements($it)", handles(index.classesImplementing(it))) }
                }
                superTypes.forEach {
                    add { Seeded("extends($it)", handles(index.classesExtending(it))) }
                }
                instanceFieldTypes.forEach {
                    add {
                        Seeded(
                            "hasInstanceField($it)",
                            handles(index.classesWithInstanceFieldType(it)),
                        )
                    }
                }
            }
        }
        return QueryPlan(seeds, "class(es)", { handles -> handles.map(::DexClass) }) {
            CandidatePool(
                index.allClasses,
                listOf(
                    "no selective constraints; considering all ${index.allClasses.size} classes"
                ),
                index.allClasses.size,
                index.allClasses.take(12),
                null,
            )
        }
    }

    override fun prefetch(runtime: PatchRuntime, candidates: List<DexClass>) =
        runtime.run.cache.prefetchClasses(candidates.map { it.handle })

    override fun rankScope(index: SearchIndex, value: DexClass) = ClassRankScopeImpl(index, value)

    override fun describe(value: DexClass) = value.descriptor

    override fun mismatch(value: DexClass, runtime: PatchRuntime): String? {
        val index = runtime.index
        if (isHiddenExtension(value.info.dexIndex.toInt(), index))
            return "defined by a bundle extension"
        if (stringValues.any { !index.classHasString(value, it) }) return "missing required string"
        if (enumNames.isNotEmpty() && !index.enumConstants(value).keys.containsAll(enumNames))
            return "missing enum constant"
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
        if (enumNames.isNotEmpty()) add("matched enum constants ${enumNames.joinToString()}")
        if (instanceFieldTypes.isNotEmpty())
            add("matched instance fields ${instanceFieldTypes.joinToString()}")
        if (superTypes.isNotEmpty()) add("matched superclass ${superTypes.joinToString()}")
        if (interfaceTypes.isNotEmpty()) add("matched interfaces ${interfaceTypes.joinToString()}")
        if (sourceFiles.isNotEmpty()) add("matched source file ${sourceFiles.joinToString()}")
        if (predicates.isNotEmpty()) add("matched ${predicates.size} custom predicate(s)")
        extensionReason()?.let(::add)
    }
}

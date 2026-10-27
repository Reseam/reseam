// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.isSet
import app.reseam.patch.native.allMethodHandles
import app.reseam.patch.native.classMethodsByName
import app.reseam.patch.native.findClass
import app.reseam.patch.native.findClassesExtending
import app.reseam.patch.native.findClassesImplementing
import app.reseam.patch.native.findClassesWithInstanceField
import app.reseam.patch.native.getAllClasses
import app.reseam.patch.native.isAddedDex
import app.reseam.patch.run.Memos
import app.reseam.patch.run.RunCache
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

private data class ClassMethods(val owner: String, val name: String?)

/** The run's classes and methods by what queries look them up by. */
internal class SearchIndex(private val cache: RunCache) {
    private val memos = cache.memos
    private val classList = memos.keptOnEdit<Unit, List<DexClass>>()
    private val methodList = memos.keptOnEdit<Unit, List<Method>>()
    private val classesBySourceFile = memos.keptOnEdit<Unit, Map<String?, List<DexClass>>>()
    private val classByDescriptor = memos.keptOnEdit<String, DexClass?>()
    private val methodBySignature = memos.keptOnEdit<MethodSignature, Method?>()
    private val methodsByClass = memos.keptOnEdit<ClassMethods, List<Method>>()
    private val inheritedByClass = memos.keptOnEdit<ClassMethods, List<Method>>()
    private val classesByString = memos.clearedOnEdit<String, Set<DexClass>>()
    private val classesByField = memos.keptOnEdit<String, Set<DexClass>>()
    private val subclasses = memos.keptOnEdit<String, Set<DexClass>>()
    private val implementers = memos.keptOnEdit<String, Set<DexClass>>()
    private val enumsByClass = memos.clearedOnEdit<UInt, Map<String, FieldRef>>()
    private val addedDex = memos.keptOnEdit<Int, Boolean>()

    val seeds = MethodSeeds(memos)
    val references = References(memos, this)

    val allClasses: List<DexClass>
        get() = classList.getOrPut(Unit) { getAllClasses().map(::DexClass) }

    val allMethods: List<Method>
        get() = methodList.getOrPut(Unit) { allMethodHandles().map(::Method) }

    /** True for a DEX the patcher linked in, which is every extension the bundle ships. */
    fun isExtensionDex(dexIndex: Int): Boolean =
        addedDex.getOrPut(dexIndex) { isAddedDex(dexIndex.toUInt()) }

    fun classFor(descriptor: String): DexClass? =
        classByDescriptor.getOrPut(descriptor) { findClass(descriptor)?.let(::DexClass) }

    fun requireClass(descriptor: String): DexClass =
        classFor(descriptor) ?: error("Class not found: $descriptor")

    fun methodFor(ref: MethodRef): Method? = methodFor(ref.signature)

    /**
     * The method a reference names, resolved the way the runtime resolves it: the class's own
     * declaration, else the nearest one it inherits.
     */
    fun methodFor(signature: MethodSignature): Method? =
        methodBySignature.getOrPut(signature) {
            val named = { methods: List<Method> ->
                methods.firstOrNull { it.name == signature.name && it.proto == signature.proto }
            }
            named(methodsInClass(signature.owner, signature.name))
                ?: named(inheritedMethods(signature.owner, signature.name))
        }

    fun methodsInClass(descriptor: String, name: String? = null): List<Method> =
        methodsByClass.getOrPut(ClassMethods(descriptor, name)) {
            classFor(descriptor)
                ?.let { classDef ->
                    if (name == null) classDef.methods
                    else classMethodsByName(classDef.handle, name).map(::Method)
                }
                .orEmpty()
        }

    /**
     * Methods this class inherits from the app classes it extends, nearest first, minus the ones it
     * or a nearer ancestor overrides. Constructors and private declarations are not inherited.
     */
    fun inheritedMethods(descriptor: String, name: String? = null): List<Method> =
        inheritedByClass.getOrPut(ClassMethods(descriptor, name)) {
            val seen =
                methodsInClass(descriptor, name).mapTo(mutableSetOf()) { it.name to it.proto }
            classFor(descriptor)?.superclassChain.orEmpty().flatMap { ancestor ->
                methodsInClass(ancestor.descriptor, name).filter {
                    it.name != "<init>" &&
                        !AccessFlags.PRIVATE.isSet(it.info.accessFlags) &&
                        seen.add(it.name to it.proto)
                }
            }
        }

    fun classesWithString(value: String): Set<DexClass> =
        classesByString.getOrPut(value) {
            seeds
                .withStrings(listOf(value))
                .asSequence()
                .map { Method(it).owner }
                .distinct()
                .mapNotNull(::classFor)
                .toCollection(linkedSetOf())
        }

    fun classHasString(classDef: DexClass, value: String): Boolean =
        classDef.methods.any { seeds.hasString(it, value) }

    /** An enum's constants by the name each passes to its constructor; empty for other classes. */
    fun enumConstants(classDef: DexClass): Map<String, FieldRef> =
        enumsByClass.getOrPut(classDef.handle) { readEnumConstants(classDef, this) }

    fun classesWithSourceFile(name: String): List<DexClass> =
        classesBySourceFile
            .getOrPut(Unit) {
                cache.prefetchClasses(allClasses.map { it.handle })
                allClasses.groupBy { it.sourceFile }
            }[name]
            .orEmpty()

    fun classesWithInstanceFieldType(type: String): Set<DexClass> =
        classesBy(classesByField, type, ::findClassesWithInstanceField)

    fun classesExtending(type: String): Set<DexClass> =
        classesBy(subclasses, type, ::findClassesExtending)

    fun classesImplementing(type: String): Set<DexClass> =
        classesBy(implementers, type, ::findClassesImplementing)

    fun classExtends(classDef: DexClass, type: String): Boolean =
        classDef.superclass == type || classDef.superclassChain.any { it.descriptor == type }

    private fun classesBy(
        memo: Memos.Memo<String, Set<DexClass>>,
        type: String,
        find: (String) -> UIntArray,
    ): Set<DexClass> =
        memo.getOrPut(type) { find(type).map(::DexClass).toCollection(linkedSetOf()) }
}

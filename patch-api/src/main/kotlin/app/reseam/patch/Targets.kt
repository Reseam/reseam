// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

class MethodTarget
internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<Method>,
) : Target<Method>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<Method> = resolver(runtime)

    val method: Method
        get() = resolved

    val owner: String
        get() = method.owner

    val name: String
        get() = method.name

    val proto: String
        get() = method.proto

    val returnType: String
        get() = method.returnType

    val parameterTypes: List<String>
        get() = method.parameterTypes

    val descriptor: String
        get() = method.descriptor

    val ref: MethodRef
        get() = MethodRef(owner, name, proto)

    internal companion object {
        fun of(method: Method, debugName: String? = null): MethodTarget =
            MethodTarget(debugName) {
                Resolution.direct(method, debugName ?: method.descriptor, method.descriptor)
            }
    }
}

class MethodsTarget
internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<List<Method>>,
) : Target<List<Method>>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<List<Method>> = resolver(runtime)

    val all: List<MethodTarget>
        get() = resolved.map { MethodTarget.of(it, debugName) }

    fun forEach(block: MethodTarget.() -> Unit) = all.forEach(block)

    /**
     * A deferred target for the one method matching [predicate].
     *
     * Creating the target does not resolve candidates or run the predicate, so it may be declared
     * at top level. On first use in a patch runtime, the predicate runs for each candidate with
     * that runtime active and may read other targets. The selected method is cached for the rest of
     * that runtime.
     *
     * @throws IllegalStateException when the target resolves and zero or multiple candidates match,
     *   with the match count and all candidate descriptors.
     */
    fun single(predicate: MethodTarget.() -> Boolean = { true }): MethodTarget =
        MethodTarget(debugName) { runtime ->
            val candidates = runtime.resolve(this).value.map { MethodTarget.of(it, debugName) }
            val matches = candidates.filter(predicate)
            val winner =
                matches.singleOrNull()
                    ?: error(
                        "$label: expected one method, got ${matches.size} of ${candidates.size}: ${candidates.joinToString { it.descriptor }}"
                    )
            runtime.resolve(winner)
        }
}

class ClassTarget
internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<DexClass>,
) : Target<DexClass>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<DexClass> = resolver(runtime)

    val classDef: DexClass
        get() = resolved

    val descriptor: String
        get() = classDef.descriptor

    internal companion object {
        fun of(classDef: DexClass, debugName: String? = null): ClassTarget =
            ClassTarget(debugName) {
                Resolution.direct(classDef, debugName ?: classDef.descriptor, classDef.descriptor)
            }
    }
}

class FieldTarget
internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<FieldRef>,
) : Target<FieldRef>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<FieldRef> = resolver(runtime)

    val ref: FieldRef
        get() = resolved

    val owner: String
        get() = ref.definingClass

    val name: String
        get() = ref.name

    val type: String
        get() = ref.fieldType

    internal companion object {
        fun of(ref: FieldRef, debugName: String? = null): FieldTarget =
            FieldTarget(debugName) {
                resolvedField(debugName ?: "${ref.definingClass}.${ref.name}", ref)
            }
    }
}

internal fun resolvedField(name: String, ref: FieldRef): Resolution<FieldRef> =
    Resolution.direct(ref, name, "${ref.definingClass}.${ref.name}:${ref.fieldType}")

/**
 * The one method satisfying the query. More than one match is an error unless the query ranks or
 * takes `first()`. The block runs when the target resolves, so it may read other targets.
 */
fun method(debugName: String? = null, block: MethodQuery.() -> Unit): MethodTarget =
    MethodTarget(debugName) { runtime ->
        MethodQuerySpec().apply(block).resolveOne(runtime, debugName)
    }

/** Every method satisfying the query, best ranked first. */
fun methods(debugName: String? = null, block: MethodQuery.() -> Unit): MethodsTarget =
    MethodsTarget(debugName) { runtime ->
        MethodQuerySpec().apply(block).resolveAll(runtime, debugName)
    }

/**
 * A method found by any means: the block runs with the patch runtime and may use the `dex` layer.
 */
fun methodTarget(debugName: String, resolve: PatchRuntime.() -> Method): MethodTarget =
    MethodTarget(debugName) { runtime ->
        runtime.resolve().let { Resolution.direct(it, debugName, it.descriptor) }
    }

fun classTarget(debugName: String, resolve: PatchRuntime.() -> DexClass): ClassTarget =
    ClassTarget(debugName) { runtime ->
        runtime.resolve().let { Resolution.direct(it, debugName, it.descriptor) }
    }

fun fieldTarget(debugName: String, resolve: PatchRuntime.() -> FieldRef): FieldTarget =
    FieldTarget(debugName) { runtime ->
        runtime.resolve().let {
            resolvedField(debugName, it)
        }
    }

/** A class by name, dotted or as a descriptor. */
fun klass(name: String): ClassTarget {
    val desc = descriptor(name)
    return ClassTarget(className(desc)) { runtime ->
        val found = runtime.index.requireClass(desc)
        Resolution.direct(found, className(desc), desc)
    }
}

/** The one class satisfying the query. */
fun klass(debugName: String, block: ClassQuery.() -> Unit): ClassTarget =
    ClassTarget(debugName) { runtime ->
        ClassQuerySpec().apply(block).resolveOne(runtime, debugName)
    }

/**
 * A method of this class by name, narrowed by the query when overloaded. `inherited` includes
 * methods inherited from app classes.
 */
fun ClassTarget.method(
    name: String,
    inherited: Boolean = false,
    block: MethodQuery.() -> Unit = {},
): MethodTarget {
    val owner = this
    return app.reseam.patch.method("${owner.label}.$name") {
        inClass(owner, inherited)
        name(name)
        block()
    }
}

fun ClassTarget.methods(
    debugName: String? = null,
    inherited: Boolean = false,
    block: MethodQuery.() -> Unit,
): MethodsTarget {
    val owner = this
    return app.reseam.patch.methods(debugName ?: owner.label) {
        inClass(owner, inherited)
        block()
    }
}

/**
 * The one field this method reads or writes that matches the block, which may read other targets.
 */
fun MethodTarget.field(debugName: String? = null, block: FieldRefMatch.() -> Unit): FieldTarget {
    val owner = this
    val label = debugName ?: "${owner.label}.field"
    return FieldTarget(label) { runtime ->
        val method = runtime.resolve(owner).value
        val spec = FieldRefMatchSpec().apply(block)
        val matches = runtime.index.fieldRefsOf(method).filter(spec::matches)
        val ref =
            matches.singleOrNull()
                ?: error(
                    "$label: expected one matching field in ${method.descriptor}, got ${matches.size}: " +
                        matches.joinToString { "${it.definingClass}.${it.name}" }
                )
        resolvedField(label, ref)
    }
}

/** A field of this class by name. */
fun ClassTarget.field(name: String): FieldTarget {
    val owner = this
    return FieldTarget("${owner.label}.$name") { runtime ->
        val classDef = runtime.resolve(owner).value
        val ref = classDef.field(name) ?: error("${classDef.descriptor} has no field named $name")
        resolvedField("${owner.label}.$name", ref)
    }
}

/**
 * The constant of this enum whose `name()` is [name]. Obfuscation renames the constant's field but
 * keeps the name it passes to the enum's constructor, so the target finds the field that holds the
 * instance constructed with [name].
 *
 * ```kotlin
 * private val premium = planEnum.enumConstant("PREMIUM")
 *
 * planGetter.replace { returnValue(staticField(premium)) }
 * ```
 */
fun ClassTarget.enumConstant(name: String): FieldTarget {
    val owner = this
    val label = "${owner.label}.$name"
    return FieldTarget(label) { runtime ->
        val classDef = runtime.resolve(owner).value
        val ref =
            runtime.index.enumConstants(classDef)[name]
                ?: error("${classDef.descriptor} has no enum constant named $name")
        resolvedField(label, ref)
    }
}

/** The one instance field of this class with `type`. */
fun ClassTarget.fieldOfType(type: String): FieldTarget {
    val owner = this
    val desc = descriptor(type)
    return FieldTarget("${owner.label}:${type}") { runtime ->
        val ref = runtime.resolve(owner).value.instanceFieldOfType(desc)
        resolvedField("${owner.label}:$type", ref)
    }
}

/** A field reference that is not looked up; use for fields on classes the bundle itself adds. */
fun field(owner: String, name: String, type: String): FieldTarget =
    FieldTarget.of(FieldRef(descriptor(owner), name, descriptor(type)))

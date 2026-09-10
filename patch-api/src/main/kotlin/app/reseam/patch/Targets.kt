// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.buildInstructions

class MethodTarget internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<Method>,
) : Target<Method>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<Method> = resolver(runtime)

    val method: Method get() = resolved
    val owner: String get() = method.owner
    val name: String get() = method.name
    val proto: String get() = method.proto
    val returnType: String get() = method.returnType
    val parameterTypes: List<String> get() = method.parameterTypes
    val descriptor: String get() = method.descriptor
    val ref: MethodRef get() = MethodRef(owner, name, proto)

    internal companion object {
        fun of(method: Method, debugName: String? = null): MethodTarget =
            MethodTarget(debugName) { Resolution(method, wrapped(debugName ?: method.descriptor, method.descriptor)) }
    }
}

class MethodsTarget internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<List<Method>>,
) : Target<List<Method>>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<List<Method>> = resolver(runtime)

    val all: List<MethodTarget>
        get() = resolved.map { MethodTarget.of(it, debugName) }

    fun forEach(block: MethodTarget.() -> Unit) = all.forEach(block)

    /** The one method matching `predicate`; fails listing the candidates otherwise. */
    fun single(predicate: MethodTarget.() -> Boolean): MethodTarget {
        val matches = all.filter(predicate)
        return matches.singleOrNull() ?: error(
            "$label: expected one method, got ${matches.size} of ${all.size}: ${all.joinToString { it.descriptor }}",
        )
    }
}

class ClassTarget internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<DexClass>,
) : Target<DexClass>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<DexClass> = resolver(runtime)

    val classDef: DexClass get() = resolved
    val descriptor: String get() = classDef.descriptor

    internal companion object {
        fun of(classDef: DexClass, debugName: String? = null): ClassTarget =
            ClassTarget(debugName) { Resolution(classDef, wrapped(debugName ?: classDef.descriptor, classDef.descriptor)) }
    }
}

class FieldTarget internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<FieldRef>,
) : Target<FieldRef>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<FieldRef> = resolver(runtime)

    val ref: FieldRef get() = resolved
    val owner: String get() = ref.definingClass
    val name: String get() = ref.name
    val type: String get() = ref.fieldType

    internal companion object {
        fun of(ref: FieldRef, debugName: String? = null): FieldTarget =
            FieldTarget(debugName) { Resolution(ref, wrapped(debugName ?: "${ref.definingClass}.${ref.name}", "${ref.definingClass}.${ref.name}:${ref.fieldType}")) }
    }
}

internal fun wrapped(name: String, winner: String): MatchReport =
    SearchMatchReport(name, winner, considered = 1, reasons = listOf("resolved directly"), nearMisses = emptyList())

/**
 * The one method satisfying the query. More than one match is an error unless
 * the query ranks or takes `first()`. The block runs when the target resolves,
 * so it may read other targets.
 */
fun method(debugName: String? = null, block: MethodQuery.() -> Unit): MethodTarget =
    MethodTarget(debugName) { runtime -> MethodQuerySpec().apply(block).resolveOne(runtime, debugName) }

/** Every method satisfying the query, best ranked first. */
fun methods(debugName: String? = null, block: MethodQuery.() -> Unit): MethodsTarget =
    MethodsTarget(debugName) { runtime -> MethodQuerySpec().apply(block).resolveAll(runtime, debugName) }

/** A method found by any means: the block runs with the patch runtime and may use the `dex` layer. */
fun methodTarget(debugName: String, resolve: PatchRuntime.() -> Method): MethodTarget =
    MethodTarget(debugName) { runtime -> runtime.resolve().let { Resolution(it, wrapped(debugName, it.descriptor)) } }

fun classTarget(debugName: String, resolve: PatchRuntime.() -> DexClass): ClassTarget =
    ClassTarget(debugName) { runtime -> runtime.resolve().let { Resolution(it, wrapped(debugName, it.descriptor)) } }

fun fieldTarget(debugName: String, resolve: PatchRuntime.() -> FieldRef): FieldTarget =
    FieldTarget(debugName) { runtime -> runtime.resolve().let { Resolution(it, wrapped(debugName, "${it.definingClass}.${it.name}:${it.fieldType}")) } }

/** A class by name, dotted or as a descriptor. */
fun klass(name: String): ClassTarget {
    val desc = descriptor(name)
    return ClassTarget(className(desc)) { runtime ->
        val found = runtime.index.classFor(desc) ?: error("Class not found: $desc")
        Resolution(found, wrapped(className(desc), desc))
    }
}

/** The one class satisfying the query. */
fun klass(debugName: String, block: ClassQuery.() -> Unit): ClassTarget =
    ClassTarget(debugName) { runtime -> ClassQuerySpec().apply(block).resolveOne(runtime, debugName) }

/** A method of this class by name, narrowed by the query when overloaded. */
fun ClassTarget.method(name: String, block: MethodQuery.() -> Unit = {}): MethodTarget {
    val owner = this
    return app.reseam.patch.method("${owner.label}.$name") {
        inClass(owner)
        name(name)
        block()
    }
}

fun ClassTarget.methods(debugName: String? = null, block: MethodQuery.() -> Unit): MethodsTarget {
    val owner = this
    return app.reseam.patch.methods(debugName ?: owner.label) {
        inClass(owner)
        block()
    }
}

/** A field of this class by name. */
fun ClassTarget.field(name: String): FieldTarget {
    val owner = this
    return FieldTarget("${owner.label}.$name") { runtime ->
        val classDef = runtime.resolve(owner).value
        val ref = classDef.field(name) ?: error("${classDef.descriptor} has no field named $name")
        Resolution(ref, wrapped("${owner.label}.$name", "${ref.definingClass}.${ref.name}:${ref.fieldType}"))
    }
}

/** The one instance field of this class with `type`. */
fun ClassTarget.fieldOfType(type: String): FieldTarget {
    val owner = this
    val desc = descriptor(type)
    return FieldTarget("${owner.label}:${type}") { runtime ->
        val classDef = runtime.resolve(owner).value
        val matches = classDef.instanceFields.filter { it.fieldType == desc }
        val field = matches.singleOrNull() ?: error(
            "Expected exactly one instance field of type $desc on ${classDef.descriptor}, found ${matches.size}",
        )
        val ref = FieldRef(field.classDescriptor, field.name, field.fieldType)
        Resolution(ref, wrapped("${owner.label}:$type", "${ref.definingClass}.${ref.name}:${ref.fieldType}"))
    }
}

/** A field reference that is not looked up; use for fields on classes the bundle itself adds. */
fun field(owner: String, name: String, type: String): FieldTarget =
    FieldTarget.of(FieldRef(descriptor(owner), name, descriptor(type)))

/**
 * `onCreate()` of the app's `Application` subclass named in the manifest.
 * Added when the class does not override it, so there is always a method
 * that runs once at process start.
 */
val appEntry: MethodTarget = MethodTarget("appEntry") { runtime ->
    val name = runtime.manifest.applicationClass
        ?: error("The manifest names no <application android:name>, so there is no app entry point to hook")
    val desc = descriptor(name)
    val classDef = runtime.index.classFor(desc) ?: error("Application class $desc is not in the app")
    val existing = classDef.method("onCreate", "()V")
    val method = existing ?: classDef.addMethod(
        NewMethod(
            name = "onCreate",
            proto = "()V",
            accessFlags = AccessFlags.PUBLIC.toUInt(),
            registersSize = 1u,
            insSize = 1u,
            outsSize = 1u,
            instructions = buildInstructions {
                invokeSuper(classDef.superclass ?: Type.Application, "onCreate", "()V", 0)
                returnVoid()
            },
            tries = emptyList(),
            catchHandlers = emptyList(),
        ),
    )
    Resolution(
        method,
        SearchMatchReport(
            "appEntry",
            method.descriptor,
            considered = 1,
            reasons = listOf(if (existing != null) "overrides onCreate" else "added onCreate calling super"),
            nearMisses = emptyList(),
        ),
    )
}

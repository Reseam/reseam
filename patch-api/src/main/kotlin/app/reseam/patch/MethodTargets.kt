// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.Method
import app.reseam.patch.run.Resolution
import app.reseam.patch.search.MethodQuerySpec
import app.reseam.patch.types.MethodRef
import app.reseam.patch.types.NewMethod

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

    /** Runs [block] with each matching method as its receiver. */
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
 * Method [name] [proto] of this class with [body] as its code. The class gains the method when it
 * does not declare it yet, so patches that share a helper method can each define it.
 */
fun ClassTarget.addMethod(
    name: String,
    proto: String,
    accessFlags: Int =
        if (classDef.isInterface) AccessFlags.PUBLIC else AccessFlags.PUBLIC or AccessFlags.FINAL,
    body: CodeScope.() -> Unit,
): MethodTarget {
    // Declared abstract, the method has no code until its body replaces it.
    val method =
        classDef.method(name, proto)
            ?: classDef.addMethod(
                NewMethod(
                    name = name,
                    proto = proto,
                    accessFlags = (accessFlags or AccessFlags.ABSTRACT).toUInt(),
                    registersSize = 0u,
                    insSize = 0u,
                    outsSize = 0u,
                    instructions = emptyList(),
                    tries = emptyList(),
                    catchHandlers = emptyList(),
                )
            )
    return MethodTarget.of(method, "$label.$name").also { it.replace(body) }
}

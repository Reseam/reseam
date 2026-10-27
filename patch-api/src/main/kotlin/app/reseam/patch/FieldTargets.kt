// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.fieldRef
import app.reseam.patch.run.Resolution
import app.reseam.patch.search.FieldRefMatchSpec
import app.reseam.patch.types.FieldRef

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

fun fieldTarget(debugName: String, resolve: PatchRuntime.() -> FieldRef): FieldTarget =
    FieldTarget(debugName) { runtime ->
        runtime.resolve().let {
            resolvedField(debugName, it)
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
        val pattern = FieldRefMatchSpec().apply(block).pattern
        val insns = method.instructions
        val matches =
            runtime.index.references
                .accessesIn(method, pattern)
                .map { checkNotNull(insns[it].fieldRef) }
                .distinct()
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

internal fun resolvedField(name: String, ref: FieldRef): Resolution<FieldRef> =
    Resolution.direct(ref, name, "${ref.definingClass}.${ref.name}:${ref.fieldType}")

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.run.Resolution
import app.reseam.patch.types.MethodRef
import kotlin.properties.PropertyDelegateProvider
import kotlin.properties.ReadOnlyProperty
import kotlin.reflect.KProperty

/**
 * A class the bundle ships as an extension. Declaring one names its members once; the engine links
 * the extension into the app the first time a patch refers to it.
 *
 * ```kotlin
 * object AdBlocker : ExtClass("app.example.ext.AdBlocker") {
 *     val isAd by static(Type.Object, returns = Type.Boolean)
 * }
 * ```
 */
open class ExtClass(name: String) {
    val descriptor: String = descriptor(name)
    val target: ClassTarget by lazy { klass(descriptor) }

    /** Declares a static method named after the property, or [name]. */
    fun static(
        vararg params: String,
        returns: String = Type.Void,
        name: String? = null,
    ): MemberDelegate<ExtMethod> =
        MemberDelegate(name) { ExtMethod(descriptor, it, proto(returns, *params), isStatic = true) }

    /** Declares an instance method named after the property, or [name]. */
    fun method(
        vararg params: String,
        returns: String = Type.Void,
        name: String? = null,
    ): MemberDelegate<ExtMethod> =
        MemberDelegate(name) {
            ExtMethod(descriptor, it, proto(returns, *params), isStatic = false)
        }

    /** Declares a field named after the property, or [name]. */
    fun field(type: String, name: String? = null): MemberDelegate<FieldTarget> =
        MemberDelegate(name) { field(descriptor, it, type) }

    override fun toString() = descriptor
}

/** Declares an extension member named after its property unless a name is given. */
class MemberDelegate<T>
internal constructor(
    private val name: String?,
    private val create: (name: String) -> T,
) : PropertyDelegateProvider<Any?, ReadOnlyProperty<Any?, T>> {
    override fun provideDelegate(thisRef: Any?, property: KProperty<*>): ReadOnlyProperty<Any?, T> {
        val member = create(name ?: property.name)
        return ReadOnlyProperty { _, _ -> member }
    }
}

class ExtMethod
internal constructor(
    val owner: String,
    val name: String,
    val proto: String,
    val isStatic: Boolean,
) {
    val ref: MethodRef = MethodRef(owner, name, proto)

    val target: MethodTarget by lazy {
        MethodTarget("${className(owner)}.$name") { runtime ->
            val method = runtime.index.methodFor(ref) ?: error(missing(runtime))
            Resolution.direct(method, "${className(owner)}.$name", method.descriptor)
        }
    }

    private fun missing(runtime: PatchRuntime): String {
        val declared =
            runtime.index.methodsInClass(owner).filter { it.name == name }.map { it.proto }
        val hint =
            if (declared.isEmpty()) ""
            else "; ${className(owner)} declares " + declared.joinToString(" and ") { "$name$it" }
        return "$owner->$name$proto is not in the app or any extension$hint"
    }

    /** Replaces the method's body with emitted code. */
    fun implement(block: CodeScope.() -> Unit) = target.replace(block)

    override fun toString() = "$owner->$name$proto"
}

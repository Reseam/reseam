// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef
import kotlin.properties.PropertyDelegateProvider
import kotlin.properties.ReadOnlyProperty
import kotlin.reflect.KProperty

/**
 * Code a patch adds to a method. Values are [ValueRef]s; the engine assigns registers and picks
 * instruction encodings. Kotlin control flow runs while the patch is applied; `when*` blocks become
 * branches in the app.
 */
interface CodeScope {
    /** The receiver; saved at entry for a method-level [after] hook. */
    val thisObject: ValueRef

    /** The parameter value; saved at entry for a method-level [after] hook. */
    fun param(index: Int): ValueRef

    fun paramOfType(type: String): ValueRef

    val lastParam: ValueRef

    /** A value captured by a [PointTarget.captureAs], or `result` inside [after]. */
    fun capture(name: String): ValueRef

    /** The register [MethodTarget.reserveLocal] holds for this method, as a value of this block. */
    fun local(slot: MethodLocal): ValueRef

    fun int(value: Int): ValueRef

    fun long(value: Long): ValueRef

    fun bool(value: Boolean): ValueRef

    fun string(value: String): ValueRef

    val nullObject: ValueRef

    fun enumValue(type: String, name: String): ValueRef

    fun staticField(field: FieldTarget): ValueRef

    fun staticField(field: FieldRef): ValueRef

    fun setStatic(field: FieldTarget, value: ValueRef)

    fun setStatic(field: FieldRef, value: ValueRef)

    fun newInstance(type: String, ctorProto: String = "()V", vararg args: ValueRef): ValueRef

    fun call(method: ExtMethod, vararg args: ValueRef): ValueRef

    fun call(method: MethodTarget, vararg args: ValueRef): ValueRef

    fun callStatic(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef

    /** Runs [block] for a nonzero integral value or a non-null reference. */
    fun whenTrue(value: ValueRef, block: CodeScope.() -> Unit): Otherwise

    /** Runs [block] for a zero integral value or a null reference. */
    fun whenFalse(value: ValueRef, block: CodeScope.() -> Unit): Otherwise

    /** Runs [block] for a null reference; primitive values are rejected. */
    fun whenNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise

    /** Runs [block] for a non-null reference; primitive values are rejected. */
    fun whenNotNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise

    /**
     * Compares integral values, reference identity, or matching float, double or long values.
     * Floating-point NaNs compare unequal; positive and negative zero compare equal. Incompatible
     * operand types are rejected.
     */
    fun whenEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise

    /** The inverse of [whenEqual], including floating-point NaNs. */
    fun whenNotEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise

    /** Runs `block` when `value` is an instance of `type`; null is not an instance of anything. */
    fun whenInstanceOf(value: ValueRef, type: String, block: CodeScope.() -> Unit): Otherwise

    fun returnVoid()

    fun returnValue(value: ValueRef)

    fun returnTrue()

    fun returnFalse()

    fun returnNull()
}

interface Otherwise {
    infix fun otherwise(block: CodeScope.() -> Unit)
}

interface ValueRef {
    val type: String

    fun cast(type: String): ValueRef

    fun field(field: FieldTarget): ValueRef

    fun field(ref: FieldRef): ValueRef

    /** The one instance field of `type` on this value's class. */
    fun fieldOfType(type: String): ValueRef

    fun set(field: FieldTarget, value: ValueRef)

    fun set(ref: FieldRef, value: ValueRef)

    /**
     * Overwrites this value in place: a parameter, a capture, or an earlier result. A value read
     * from a field is a copy, so assigning it fails; write the field with [set] or
     * [CodeScope.setStatic].
     */
    fun assign(value: ValueRef)

    fun call(method: ExtMethod, vararg args: ValueRef): ValueRef

    fun call(method: MethodTarget, vararg args: ValueRef): ValueRef

    fun callVirtual(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef

    fun callInterface(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef

    /** `List.size()` of this value. */
    fun size(): ValueRef

    /** `List.get(index)` of this value. */
    operator fun get(index: ValueRef): ValueRef

    operator fun plus(other: ValueRef): ValueRef

    operator fun minus(other: ValueRef): ValueRef
}

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
            Resolution(method, wrapped("${className(owner)}.$name", method.descriptor))
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

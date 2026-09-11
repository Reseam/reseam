// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

/**
 * Code a patch adds to a method. Values are [ValueRef]s; the engine assigns
 * registers and picks instruction encodings. Kotlin control flow runs while
 * the patch is applied; `when*` blocks become branches in the app.
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

    fun int(value: Int): ValueRef
    fun long(value: Long): ValueRef
    fun bool(value: Boolean): ValueRef
    fun string(value: String): ValueRef
    val nullObject: ValueRef
    fun enumValue(type: String, name: String): ValueRef
    fun staticField(field: FieldTarget): ValueRef
    fun staticField(field: FieldRef): ValueRef
    fun newInstance(type: String, ctorProto: String = "()V", vararg args: ValueRef): ValueRef

    fun call(method: ExtMethod, vararg args: ValueRef): ValueRef
    fun call(method: MethodTarget, vararg args: ValueRef): ValueRef
    fun callStatic(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef

    fun whenTrue(value: ValueRef, block: CodeScope.() -> Unit): Otherwise
    fun whenFalse(value: ValueRef, block: CodeScope.() -> Unit): Otherwise
    fun whenNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise
    fun whenNotNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise
    fun whenEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise
    fun whenNotEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise

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
    /** Overwrites this value in place: a parameter, a capture, or an earlier result. */
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
 * A class the bundle ships as an extension. Declaring one names its methods
 * once; the engine links the extension into the app the first time a patch
 * refers to it.
 */
open class ExtClass(name: String) {
    val descriptor: String = descriptor(name)
    val target: ClassTarget by lazy { klass(descriptor) }

    fun static(name: String, vararg params: String, returns: String = Type.Void): ExtMethod =
        ExtMethod(descriptor, name, proto(returns, *params), isStatic = true)

    fun method(name: String, vararg params: String, returns: String = Type.Void): ExtMethod =
        ExtMethod(descriptor, name, proto(returns, *params), isStatic = false)

    fun field(name: String, type: String): FieldTarget = field(descriptor, name, type)

    override fun toString() = descriptor
}

class ExtMethod internal constructor(
    val owner: String,
    val name: String,
    val proto: String,
    val isStatic: Boolean,
) {
    val ref: MethodRef = MethodRef(owner, name, proto)

    val target: MethodTarget by lazy {
        MethodTarget("${className(owner)}.$name") { runtime ->
            val method = runtime.index.methodFor(ref) ?: error("$owner->$name$proto is not in the app or any extension")
            Resolution(method, wrapped("${className(owner)}.$name", method.descriptor))
        }
    }

    /** Replaces the method's body with emitted code. */
    fun implement(block: CodeScope.() -> Unit) = target.replace(block)

    override fun toString() = "$owner->$name$proto"
}

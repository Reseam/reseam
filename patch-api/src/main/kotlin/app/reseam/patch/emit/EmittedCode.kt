// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.CodeScope
import app.reseam.patch.ExtMethod
import app.reseam.patch.FieldTarget
import app.reseam.patch.MethodLocal
import app.reseam.patch.MethodTarget
import app.reseam.patch.Otherwise
import app.reseam.patch.PointTarget
import app.reseam.patch.Type
import app.reseam.patch.ValueRef
import app.reseam.patch.descriptor as descriptorOf
import app.reseam.patch.dex.InstructionBuilder
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isStatic
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.method
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

/** The [CodeScope] a patch writes a block in, recording each operation in [emitter]. */
internal class EmittedCode(val emitter: CodeEmitter) : CodeScope {
    override val thisObject: ValueRef
        get() = emitter.thisObject()

    override fun param(index: Int): ValueRef = emitter.param(index)

    override fun paramOfType(type: String): ValueRef = emitter.paramOfType(type)

    override val lastParam: ValueRef
        get() = emitter.param(emitter.method.parameterTypes.lastIndex)

    override val result: ValueRef
        get() = emitter.result()

    override fun capture(name: String): ValueRef = emitter.capture(name)

    override fun local(slot: MethodLocal): ValueRef = emitter.local(slot)

    override fun int(value: Int): ValueRef {
        val type = if (value == 0) ValueType.Zero else ValueType.Known(Type.Int)
        return constant(Type.Int, type) { constInt(0, value) }
    }

    override fun long(value: Long): ValueRef = constant(Type.Long) { constLong(0, value) }

    override fun float(value: Float): ValueRef =
        constant(Type.Float) { constInt(0, value.toRawBits()) }

    override fun double(value: Double): ValueRef =
        constant(Type.Double) { constLong(0, value.toRawBits()) }

    override fun bool(value: Boolean): ValueRef =
        constant(Type.Boolean) { constInt(0, if (value) 1 else 0) }

    override fun string(value: String): ValueRef = constant(Type.String) { constString(0, value) }

    override val nullObject: ValueRef
        get() = constant(Type.Object, ValueType.Null) { constInt(0, 0) }

    private fun constant(
        type: String,
        assignableType: ValueType = ValueType.Known(type),
        load: InstructionBuilder.() -> Unit,
    ): Value =
        emitter.temporary(type, assignableType = assignableType).also {
            emitter.emit(template(load), it)
        }

    override fun enumValue(type: String, name: String): ValueRef =
        with(emitter) {
            val owner = descriptorOf(type)
            staticField(FieldRef(owner, name, owner))
        }

    override fun staticField(field: FieldTarget): ValueRef = staticField(field.ref)

    override fun staticField(field: FieldRef): ValueRef = emitter.readStatic(field)

    override fun setStatic(field: FieldTarget, value: ValueRef) = setStatic(field.ref, value)

    override fun setStatic(field: FieldRef, value: ValueRef) =
        emitter.writeStatic(field, value.builtBy(emitter))

    override fun newInstance(type: String, ctorProto: String, vararg args: ValueRef): ValueRef =
        emitter.construct(descriptorOf(type), ctorProto, args.map { it.builtBy(emitter) })

    override fun newArray(type: String, size: ValueRef): ValueRef =
        emitter.newArray(descriptorOf(type), size.builtBy(emitter))

    override fun call(method: ExtMethod, vararg args: ValueRef): ValueRef {
        require(method.isStatic) {
            "$method is an instance method; call it on a value: receiver.call(...)"
        }
        method.target.method
        return invokeStatic(method.ref, args)
    }

    override fun call(method: MethodTarget, vararg args: ValueRef): ValueRef {
        require(method.method.isStatic) {
            "${method.descriptor} is an instance method; call it on a value: receiver.call(...)"
        }
        return invokeStatic(method.ref, args)
    }

    override fun callStatic(
        owner: String,
        name: String,
        proto: String,
        vararg args: ValueRef,
    ): ValueRef = invokeStatic(MethodRef(descriptorOf(owner), name, proto), args)

    private fun invokeStatic(ref: MethodRef, args: Array<out ValueRef>): ValueRef =
        emitter.invoke(Opcode.INVOKE_STATIC, ref, args.map { it.builtBy(emitter) })

    override fun whenTrue(value: ValueRef, block: CodeScope.() -> Unit): Otherwise =
        with(emitter) {
            val tested = value.builtBy(this)
            branch(block) { ifZero(tested, it) }
        }

    override fun whenFalse(value: ValueRef, block: CodeScope.() -> Unit): Otherwise =
        with(emitter) {
            val tested = value.builtBy(this)
            branch(block) { ifNonZero(tested, it) }
        }

    override fun whenNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise =
        whenFalse(reference(value), block)

    override fun whenNotNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise =
        whenTrue(reference(value), block)

    private fun reference(value: ValueRef): ValueRef = value.also {
        requireAssignableType(it.builtBy(emitter).assignableType, Type.Object, "null comparison")
    }

    override fun whenEqual(
        left: ValueRef,
        right: ValueRef,
        block: CodeScope.() -> Unit,
    ): Otherwise =
        emitter.equality(left.builtBy(emitter), right.builtBy(emitter), Equality.EQUAL, block)

    override fun whenNotEqual(
        left: ValueRef,
        right: ValueRef,
        block: CodeScope.() -> Unit,
    ): Otherwise =
        emitter.equality(left.builtBy(emitter), right.builtBy(emitter), Equality.NOT_EQUAL, block)

    override fun whenInstanceOf(
        value: ValueRef,
        type: String,
        block: CodeScope.() -> Unit,
    ): Otherwise =
        with(emitter) {
            val isInstance = instanceOf(value.builtBy(this), descriptorOf(type))
            branch(block) { ifZero(isInstance, it) }
        }

    override fun jumpTo(point: PointTarget) = emitter.jumpTo(point)

    override fun returnVoid() = emitter.returnFromMethod(null)

    override fun returnValue(value: ValueRef) = emitter.returnFromMethod(value.builtBy(emitter))

    override fun returnTrue() = returnValue(bool(true))

    override fun returnFalse() = returnValue(bool(false))

    override fun returnNull() = returnValue(nullObject)
}

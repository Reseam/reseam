// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.CodeScope
import app.reseam.patch.ExtMethod
import app.reseam.patch.FieldTarget
import app.reseam.patch.MethodLocal
import app.reseam.patch.MethodTarget
import app.reseam.patch.Otherwise
import app.reseam.patch.Type
import app.reseam.patch.ValueRef
import app.reseam.patch.descriptor as descriptorOf
import app.reseam.patch.dex.InstructionBuilder
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isStatic
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType
import app.reseam.patch.method
import app.reseam.patch.types.CodePlacement
import app.reseam.patch.types.CodeRegister
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

/** The [CodeScope] a patch writes a block in, recording each operation in [emitter]. */
internal class EmittedCode(val emitter: CodeEmitter) : CodeScope {
    override val thisObject: ValueRef
        get() =
            with(emitter) {
                require(!isStaticMethod) {
                    "thisObject is not available in static ${info.descriptor}"
                }
                parameter(0, info.classDescriptor)
            }

    override fun param(index: Int): ValueRef =
        with(emitter) {
            val params = method.parameterTypes
            require(index in params.indices) {
                "Parameter index $index out of bounds for ${info.descriptor}"
            }
            val offset =
                (if (isStaticMethod) 0 else 1) + params.take(index).sumOf(::registerWordCount)
            parameter(offset, params[index])
        }

    override fun paramOfType(type: String): ValueRef =
        with(emitter) {
            val wanted = descriptorOf(type)
            val index = method.parameterTypes.indexOf(wanted)
            require(index >= 0) { "No parameter of type $wanted in ${info.descriptor}" }
            param(index)
        }

    override val lastParam: ValueRef
        get() = param(emitter.method.parameterTypes.lastIndex)

    override val result: ValueRef
        get() =
            with(emitter) {
                require(placement == CodePlacement.BeforeReturns && info.returnType != Type.Void) {
                    "result exists only in a method-level after hook of a non-void method, not in ${info.descriptor}"
                }
                Value(this, CodeRegister.Returned, info.returnType)
            }

    override fun capture(name: String): ValueRef =
        with(emitter) {
            val capture =
                captures.firstOrNull { it.name == name }
                    ?: error(
                        "No capture named '$name' here; available: ${captures.joinToString { it.name }.ifEmpty { "none" }}"
                    )
            fixed(capture.register, capture.type)
        }

    override fun local(slot: MethodLocal): ValueRef =
        with(emitter) {
            require(placement != CodePlacement.Body) {
                "$slot is a register of the original body, which this block replaces"
            }
            require(!slot.lost) {
                "$slot no longer exists: the method body was replaced after it was reserved"
            }
            require(slot.method.handle == method.handle) {
                "$slot is a local of another method, not ${info.descriptor}"
            }
            fixed(slot.register, slot.type)
        }

    override fun int(value: Int): ValueRef {
        val type = if (value == 0) ValueType.Zero else ValueType.Known(Type.Int)
        return constant(Type.Int, type) { constInt(0, value) }
    }

    override fun long(value: Long): ValueRef = constant(Type.Long) { constLong(0, value) }

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
        with(emitter) {
            requireAssignableType(
                value.builtBy(this).assignableType,
                Type.Object,
                "null comparison",
            )
            whenFalse(value, block)
        }

    override fun whenNotNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise =
        with(emitter) {
            requireAssignableType(
                value.builtBy(this).assignableType,
                Type.Object,
                "null comparison",
            )
            whenTrue(value, block)
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

    override fun returnVoid() = emitter.returnFromMethod(null)

    override fun returnValue(value: ValueRef) = emitter.returnFromMethod(value.builtBy(emitter))

    override fun returnTrue() = returnValue(bool(true))

    override fun returnFalse() = returnValue(bool(false))

    override fun returnNull() = returnValue(nullObject)
}

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.ExtMethod
import app.reseam.patch.FieldTarget
import app.reseam.patch.MethodTarget
import app.reseam.patch.Type
import app.reseam.patch.ValueRef
import app.reseam.patch.descriptor as descriptorOf
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isStatic
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.method
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.CodeOperand
import app.reseam.patch.types.CodeRegister
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal class Value(
    val emitter: CodeEmitter,
    private val register: CodeRegister?,
    override val type: String,
    internal val source: FieldRef? = null,
    assignableType: ValueType = ValueType.Known(type),
) : ValueRef {
    var assignableType: ValueType = assignableType
        private set

    val wordCount: Int = registerWordCount(type)

    val operand: CodeOperand
        get() = CodeOperand(checkNotNull(register) { "a void value has no register" }, type)

    override fun assign(value: ValueRef) {
        require(source == null) {
            "cannot assign to the value read from ${source!!.definingClass}->${source.name}: it is a copy; " +
                "write the field with set(field, value) or setStatic(field, value)"
        }
        val newValue = value.builtBy(emitter)
        requireAssignableType(newValue.assignableType, type, "assignment")
        emitter.move(this, newValue)
        assignableType = ValueType.Known(type)
    }

    override fun cast(type: String): ValueRef = emitter.cast(this, descriptorOf(type))

    override fun field(field: FieldTarget): ValueRef = emitter.readField(this, field.ref)

    override fun field(ref: FieldRef): ValueRef = emitter.readField(this, ref)

    override fun fieldOfType(type: String): ValueRef =
        emitter.readField(
            this,
            ActiveRuntime.current.index
                .requireClass(this.type)
                .instanceFieldOfType(descriptorOf(type)),
        )

    override fun set(field: FieldTarget, value: ValueRef) =
        emitter.writeField(this, field.ref, value.builtBy(emitter))

    override fun set(ref: FieldRef, value: ValueRef) =
        emitter.writeField(this, ref, value.builtBy(emitter))

    override fun call(method: ExtMethod, vararg args: ValueRef): ValueRef {
        require(!method.isStatic) { "$method is static; call it without a receiver" }
        method.target.method
        return emitter.invoke(
            Opcode.INVOKE_VIRTUAL,
            method.ref,
            listOf(this) + args.map { it.builtBy(emitter) },
        )
    }

    override fun call(method: MethodTarget, vararg args: ValueRef): ValueRef {
        require(!method.method.isStatic) {
            "${method.descriptor} is static; call it without a receiver"
        }
        return emitter.invoke(
            emitter.instanceInvokeKind(method),
            method.ref,
            listOf(this) + args.map { it.builtBy(emitter) },
        )
    }

    override fun callVirtual(owner: String, name: String, proto: String, vararg args: ValueRef) =
        instanceCall(Opcode.INVOKE_VIRTUAL, owner, name, proto, args)

    override fun callInterface(owner: String, name: String, proto: String, vararg args: ValueRef) =
        instanceCall(Opcode.INVOKE_INTERFACE, owner, name, proto, args)

    override fun callSuper(owner: String, name: String, proto: String, vararg args: ValueRef) =
        instanceCall(Opcode.INVOKE_SUPER, owner, name, proto, args)

    private fun instanceCall(
        opcode: Opcode,
        owner: String,
        name: String,
        proto: String,
        args: Array<out ValueRef>,
    ): ValueRef =
        emitter.invoke(
            opcode,
            MethodRef(descriptorOf(owner), name, proto),
            listOf(this) + args.map { it.builtBy(emitter) },
        )

    override fun size(): ValueRef =
        emitter.invoke(Opcode.INVOKE_INTERFACE, MethodRef(Type.List, "size", "()I"), listOf(this))

    override fun get(index: ValueRef): ValueRef =
        emitter.invoke(
            Opcode.INVOKE_INTERFACE,
            MethodRef(Type.List, "get", "(I)Ljava/lang/Object;"),
            listOf(this, index.builtBy(emitter)),
        )

    override fun plus(other: ValueRef): ValueRef =
        emitter.arithmetic(Opcode.ADD_INT, this, other.builtBy(emitter))

    override fun minus(other: ValueRef): ValueRef =
        emitter.arithmetic(Opcode.SUB_INT, this, other.builtBy(emitter))
}

/** This value as [emitter] built it; a value of another block, or of none, is rejected. */
internal fun ValueRef.builtBy(emitter: CodeEmitter): Value {
    val value =
        this as? Value ?: error("ValueRef is only valid inside the code block it was created in")
    require(value.emitter === emitter) { "ValueRef belongs to a different code block" }
    return value
}

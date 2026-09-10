// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.InstructionBuilder
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.rangeVariant
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType
import app.reseam.patch.descriptor as descriptorOf

/** Locals a replaced body gets below its incoming parameters. */
private const val REPLACE_LOCAL_BUDGET = 16
private const val VOID_REGISTER = Int.MIN_VALUE
private const val VIRTUAL_REGISTER_STRIDE = 0x10000

internal class CompiledCode(val instructions: List<Instruction>, val localGrowth: Int)

internal class ReplacementPlan(val registersSize: Int, val outsSize: Int, val instructions: List<Instruction>)

internal enum class RegisterConstraint(val maxRegister: Int) {
    LOW(15),
    BYTE(0xFF),
    ANY(0xFFFF),
}

private class TempAllocation(val baseRegister: Int, val wordCount: Int)

private typealias Op = (InstructionBuilder, resolve: (Int) -> Int) -> Unit

/**
 * Emits [CodeScope] code into a method. Values live in virtual registers
 * that are mapped onto free or newly grown registers when the code is built,
 * so the author never picks a register.
 */
internal class CodeEmitter private constructor(
    private val method: Method,
    private val insertIndex: Int?,
    private val replaceMode: Boolean,
    private val captures: List<Capture>,
) : CodeScope {
    private val ops = mutableListOf<Op>()
    private val usedRegisters = mutableSetOf<Int>()
    private var nextReplaceLocal = 0
    private var nextTempId = 0
    private var plannedLocalGrowth = 0
    private var maxOutRegisters = 0
    private var labelCounter = 0
    private val tempAllocations = mutableMapOf<Int, TempAllocation>()
    /** Low-register copies of incoming registers, reused until the register is assigned. */
    private val lowCopies = mutableMapOf<Int, Value>()

    companion object {
        fun forInsertion(method: Method, index: Int, captures: List<Capture>) =
            CodeEmitter(method, index, replaceMode = false, captures = captures)

        fun forReplacement(method: Method) =
            CodeEmitter(method, insertIndex = null, replaceMode = true, captures = emptyList())
    }

    private val info = method.info
    private val isStaticMethod = AccessFlags.STATIC.isSet(info.accessFlags)
    private val originalRegistersSize = if (replaceMode) REPLACE_LOCAL_BUDGET + method.insSize else method.registersSize
    private val incomingBase = if (replaceMode) REPLACE_LOCAL_BUDGET else method.registersSize - method.insSize

    override val thisObject: ValueRef
        get() {
            require(!isStaticMethod) { "thisObject is not available in static ${info.descriptor}" }
            return Value(incomingBase, info.classDescriptor)
        }

    override fun param(index: Int): ValueRef {
        val params = method.parameterTypes
        require(index in params.indices) { "Parameter index $index out of bounds for ${info.descriptor}" }
        val register = incomingBase + (if (isStaticMethod) 0 else 1) + params.take(index).sumOf(::registerWordCount)
        return Value(register, params[index])
    }

    override fun paramOfType(type: String): ValueRef {
        val wanted = descriptorOf(type)
        val index = method.parameterTypes.indexOf(wanted)
        require(index >= 0) { "No parameter of type $wanted in ${info.descriptor}" }
        return param(index)
    }

    override val lastParam: ValueRef
        get() = param(method.parameterTypes.lastIndex)

    override fun capture(name: String): ValueRef {
        val capture = captures.firstOrNull { it.name == name }
            ?: error("No capture named '$name' here; available: ${captures.joinToString { it.name }.ifEmpty { "none" }}")
        return Value(capture.register, capture.type)
    }

    override fun int(value: Int): ValueRef {
        val dest = allocTemp()
        op { b, r -> b.constInt(byte(r(dest), "const"), value) }
        return Value(dest, Type.Int)
    }

    override fun long(value: Long): ValueRef {
        val dest = allocTemp(wordCount = 2)
        op { b, r -> b.constLong(byte(r(dest), "const-wide"), value) }
        return Value(dest, Type.Long)
    }

    override fun bool(value: Boolean): ValueRef {
        val dest = allocTemp()
        op { b, r -> b.constInt(byte(r(dest), "const"), if (value) 1 else 0) }
        return Value(dest, Type.Boolean)
    }

    override fun string(value: String): ValueRef {
        val dest = allocTemp()
        op { b, r -> b.constString(byte(r(dest), "const-string"), value) }
        return Value(dest, Type.String)
    }

    override val nullObject: ValueRef
        get() {
            val dest = allocTemp()
            op { b, r -> b.constInt(byte(r(dest), "const"), 0) }
            return Value(dest, Type.Object)
        }

    override fun enumValue(type: String, name: String): ValueRef {
        val owner = descriptorOf(type)
        return staticField(FieldRef(owner, name, owner))
    }

    override fun staticField(field: FieldTarget): ValueRef = staticField(field.ref)

    override fun staticField(field: FieldRef): ValueRef {
        val dest = allocTemp(registerWordCount(field.fieldType))
        op { b, r -> b.sgetTyped(byte(r(dest), "sget"), field) }
        return Value(dest, field.fieldType)
    }

    override fun newInstance(type: String, ctorProto: String, vararg args: ValueRef): ValueRef {
        val owner = descriptorOf(type)
        val dest = allocTemp()
        op { b, r -> b.newInstance(byte(r(dest), "new-instance"), owner) }
        val instance = Value(dest, owner)
        invoke(Opcode.INVOKE_DIRECT, MethodRef(owner, "<init>", ctorProto), listOf(instance) + args.map { it.impl() })
        return instance
    }

    override fun call(method: ExtMethod, vararg args: ValueRef): ValueRef {
        require(method.isStatic) { "$method is an instance method; call it on a value: receiver.call(...)" }
        return invoke(Opcode.INVOKE_STATIC, method.ref, args.map { it.impl() })
    }

    override fun call(method: MethodTarget, vararg args: ValueRef): ValueRef {
        require(method.method.isStatic) { "${method.descriptor} is an instance method; call it on a value: receiver.call(...)" }
        return invoke(Opcode.INVOKE_STATIC, method.ref, args.map { it.impl() })
    }

    override fun callStatic(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef =
        invoke(Opcode.INVOKE_STATIC, MethodRef(descriptorOf(owner), name, proto), args.map { it.impl() })

    override fun whenTrue(value: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val register = value.impl().asByte().register
        return branch(block) { elseLabel -> op { b, r -> b.ifEqz(byte(r(register), "if-eqz"), elseLabel) } }
    }

    override fun whenFalse(value: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val register = value.impl().asByte().register
        return branch(block) { elseLabel -> op { b, r -> b.ifNez(byte(r(register), "if-nez"), elseLabel) } }
    }

    override fun whenNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise = whenFalse(value, block)

    override fun whenNotNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise = whenTrue(value, block)

    override fun whenEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val (a, c) = left.impl().asLow().register to right.impl().asLow().register
        return branch(block) { elseLabel -> op { b, r -> b.ifNe(low(r(a), "if-ne A"), low(r(c), "if-ne B"), elseLabel) } }
    }

    override fun whenNotEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val (a, c) = left.impl().asLow().register to right.impl().asLow().register
        return branch(block) { elseLabel -> op { b, r -> b.ifEq(low(r(a), "if-eq A"), low(r(c), "if-eq B"), elseLabel) } }
    }

    private fun branch(block: CodeScope.() -> Unit, condition: (elseLabel: String) -> Unit): Otherwise {
        val elseLabel = nextLabel("else")
        val endLabel = nextLabel("end")
        condition(elseLabel)
        this.block()
        val elseIndex = ops.size
        label(elseLabel)
        return object : Otherwise {
            override fun otherwise(block: CodeScope.() -> Unit) {
                ops.add(elseIndex) { b, _ -> b.goto(endLabel) }
                this@CodeEmitter.block()
                label(endLabel)
            }
        }
    }

    override fun returnVoid() = op { b, _ -> b.returnVoid() }

    override fun returnValue(value: ValueRef) {
        val impl = value.impl().asByte()
        val type = impl.type
        op { b, r ->
            val register = byte(r(impl.register), "return")
            when {
                isReferenceType(type) -> b.returnObject(register)
                registerWordCount(type) == 2 -> b.returnWide(register)
                else -> b.returnValue(register)
            }
        }
    }

    override fun returnTrue() = returnValue(bool(true))
    override fun returnFalse() = returnValue(bool(false))
    override fun returnNull() = returnValue(nullObject)

    internal fun buildInsertion() = CompiledCode(build(), plannedLocalGrowth)

    internal fun buildReplacement() = ReplacementPlan(REPLACE_LOCAL_BUDGET + method.insSize, maxOutRegisters, build())

    private fun build(): List<Instruction> {
        val builder = InstructionBuilder()
        for (op in ops) op(builder, ::resolveRegister)
        return builder.build()
    }

    internal fun invoke(opcode: Opcode, ref: MethodRef, args: List<Value>): ValueRef {
        val registers = args.flatMap { it.registerWords() }
        maxOutRegisters = maxOf(maxOutRegisters, registers.size)
        emitInvoke(opcode, ref, args, registers)
        val returnType = ref.returnType
        if (returnType == Type.Void) return Value(VOID_REGISTER, Type.Void)
        val dest = allocTemp(registerWordCount(returnType))
        op { b, r -> b.moveResultTyped(byte(r(dest), "move-result"), returnType) }
        return Value(dest, returnType)
    }

    private fun emitInvoke(opcode: Opcode, ref: MethodRef, args: List<Value>, registers: List<Int>) {
        val resolved = registers.map(::resolvedForCheck)
        if (resolved.size <= 5 && resolved.all { it in 0..15 }) {
            op { b, r -> b.invoke(opcode, ref, registers.map(r)) }
            return
        }
        val range = opcode.rangeVariant ?: error("$opcode does not support invoke/range lowering")
        if (resolved.isConsecutive()) {
            op { b, r -> b.invokeRange(range, ref, r(registers.first()), registers.size) }
            return
        }
        val scratch = allocTemp(registers.size, RegisterConstraint.ANY, excludeRegisters = resolved.filter { it >= 0 }.toSet())
        val scratchWords = registerWords(scratch, registers.size)
        var dest = 0
        for (arg in args) {
            moveValue(scratchWords[dest], arg.register, arg.type)
            dest += arg.wordCount
        }
        op { b, r -> b.invokeRange(range, ref, r(scratch), registers.size) }
    }

    internal fun readField(value: Value, field: FieldRef): ValueRef {
        val dest = allocTemp(registerWordCount(field.fieldType), RegisterConstraint.LOW)
        val obj = value.asLow()
        op { b, r -> b.igetTyped(low(r(dest), "iget A"), low(r(obj.register), "iget B"), field) }
        return Value(dest, field.fieldType)
    }

    internal fun writeField(value: Value, field: FieldRef, newValue: Value) {
        val src = newValue.asLow()
        val obj = value.asLow()
        op { b, r -> b.iputTyped(low(r(src.register), "iput A"), low(r(obj.register), "iput B"), field) }
    }

    internal fun uniqueField(owner: String, type: String): FieldRef {
        val classDef = ActiveRuntime.current.index.classFor(owner) ?: error("Class not found: $owner")
        val fields = classDef.instanceFields.filter { it.fieldType == type }
        require(fields.size == 1) { "Expected exactly one instance field of type $type on $owner, found ${fields.size}" }
        return FieldRef(owner, fields.single().name, type)
    }

    internal fun instanceInvokeKind(target: MethodTarget): Opcode {
        val info = target.method.info
        return when {
            AccessFlags.PRIVATE.isSet(info.accessFlags) || AccessFlags.CONSTRUCTOR.isSet(info.accessFlags) -> Opcode.INVOKE_DIRECT
            ActiveRuntime.current.index.classFor(target.owner)?.isInterface == true -> Opcode.INVOKE_INTERFACE
            else -> Opcode.INVOKE_VIRTUAL
        }
    }

    internal fun arithmetic(opcode: Opcode, left: Value, right: Value): ValueRef {
        val dest = allocTemp()
        val a = left.asByte()
        val c = right.asByte()
        op { b, r -> b.reg3(opcode, byte(r(dest), "binop A"), byte(r(a.register), "binop B"), byte(r(c.register), "binop C")) }
        return Value(dest, Type.Int)
    }

    internal fun cast(value: Value, type: String): ValueRef {
        val target = value.asByte()
        op { b, r -> b.checkCast(byte(r(target.register), "check-cast"), type) }
        return Value(target.register, type)
    }

    internal fun label(name: String) = op { b, _ -> b.label(name) }

    internal fun goto(label: String) = op { b, _ -> b.goto(label) }

    internal fun ifZero(value: Value, label: String) {
        val v = value.asByte()
        op { b, r -> b.ifEqz(byte(r(v.register), "if-eqz"), label) }
    }

    internal fun ifNonZero(value: Value, label: String) {
        val v = value.asByte()
        op { b, r -> b.ifNez(byte(r(v.register), "if-nez"), label) }
    }

    internal fun constZero(dest: Int, type: String) = op { b, r ->
        if (registerWordCount(type) == 2) b.constLong(byte(r(dest), "const-wide"), 0) else b.constInt(byte(r(dest), "const"), 0)
    }

    internal fun instanceOf(value: Value, type: String): Value {
        val dest = allocTemp(constraint = RegisterConstraint.LOW)
        val ref = value.asLow()
        op { b, r -> b.instanceOf(low(r(dest), "instance-of A"), low(r(ref.register), "instance-of B"), type) }
        return Value(dest, Type.Boolean)
    }

    internal fun nextLabel(prefix: String): String = "${prefix}_${labelCounter++}"

    private fun op(op: Op) {
        ops += op
    }

    internal fun allocTemp(
        wordCount: Int = 1,
        constraint: RegisterConstraint = RegisterConstraint.BYTE,
        excludeRegisters: Set<Int> = emptySet(),
    ): Int {
        require(wordCount > 0)
        val tempId = nextTempId++
        val register = virtualRegister(tempId)
        val base = if (replaceMode) {
            require(nextReplaceLocal + wordCount <= REPLACE_LOCAL_BUDGET) {
                "Code exceeded the $REPLACE_LOCAL_BUDGET local registers a replaced body gets in ${info.descriptor}"
            }
            nextReplaceLocal.also { nextReplaceLocal += wordCount }
        } else {
            allocateInsertionTemp(wordCount, constraint, excludeRegisters)
        }
        tempAllocations[tempId] = TempAllocation(base, wordCount)
        return register
    }

    private fun allocateInsertionTemp(wordCount: Int, constraint: RegisterConstraint, excludeRegisters: Set<Int>): Int {
        val index = insertIndex ?: 0
        val registers = method.findContiguousFreeRegisters(index, wordCount, (usedRegisters + excludeRegisters).toList())
        if (registers.size == wordCount && registers.last() <= constraint.maxRegister) {
            usedRegisters += registers
            return registers.first()
        }
        val grownBase = incomingBase + plannedLocalGrowth
        val grownLast = grownBase + wordCount - 1
        val newRegistersSize = originalRegistersSize + plannedLocalGrowth + wordCount
        require(grownLast <= constraint.maxRegister && newRegistersSize <= UShort.MAX_VALUE.toInt()) {
            "Cannot allocate $wordCount ${constraint.name.lowercase()} scratch register(s) at ${info.descriptor}[$index]; " +
                "free candidates=$registers, plannedLocalGrowth=$plannedLocalGrowth, registersSize=$originalRegistersSize, insSize=${method.insSize}"
        }
        plannedLocalGrowth += wordCount
        usedRegisters += (grownBase..grownLast)
        return grownBase
    }

    internal fun moveValue(dest: Int, src: Int, type: String) = op { b, r -> b.moveTyped(r(dest), r(src), type) }

    internal fun registerWords(register: Int, wordCount: Int): List<Int> =
        if (isVirtualRegister(register)) {
            (0 until wordCount).map { virtualRegister(virtualRegisterId(register), it) }
        } else {
            (0 until wordCount).map { register + it }
        }

    private fun registerFits(register: Int, wordCount: Int, constraint: RegisterConstraint): Boolean {
        if (register == VOID_REGISTER) return false
        val base = resolvedForCheck(register)
        return base >= 0 && base + wordCount - 1 <= constraint.maxRegister
    }

    private fun resolvedForCheck(register: Int): Int = when {
        isVirtualRegister(register) -> tempAllocations[virtualRegisterId(register)]?.let { it.baseRegister + virtualRegisterOffset(register) } ?: -1
        !replaceMode && register >= incomingBase -> register + plannedLocalGrowth
        else -> register
    }

    private fun isShiftedPhysical(register: Int): Boolean = !replaceMode && register >= incomingBase

    private fun resolveRegister(register: Int): Int {
        require(register != VOID_REGISTER) { "a void value has no register" }
        if (isVirtualRegister(register)) {
            val allocation = tempAllocations[virtualRegisterId(register)] ?: error("unallocated virtual register $register")
            return allocation.baseRegister + virtualRegisterOffset(register)
        }
        return if (isShiftedPhysical(register)) register + plannedLocalGrowth else register
    }

    private fun low(register: Int, context: String): Int {
        require(register in 0..15) { "$context requires a 4-bit register, got v$register" }
        return register
    }

    private fun byte(register: Int, context: String): Int {
        require(register in 0..0xFF) { "$context requires an 8-bit register, got v$register" }
        return register
    }

    internal inner class Value(val register: Int, override val type: String) : ValueRef {
        val wordCount: Int = registerWordCount(type)
        val emitter: CodeEmitter get() = this@CodeEmitter

        fun registerWords(): List<Int> = registerWords(register, wordCount)

        fun asLow(): Value = fitted(RegisterConstraint.LOW)

        fun asByte(): Value = fitted(RegisterConstraint.BYTE)

        private fun fitted(constraint: RegisterConstraint): Value {
            if (registerFits(register, wordCount, constraint) && !isShiftedPhysical(register)) return this
            lowCopies[register]?.takeIf { it.type == type && registerFits(it.register, wordCount, constraint) }?.let { return it }
            val dest = allocTemp(wordCount, constraint)
            moveValue(dest, register, type)
            return Value(dest, type).also { if (!isVirtualRegister(register)) lowCopies[register] = it }
        }

        override fun assign(value: ValueRef) {
            val source = value.impl()
            require(registerWordCount(source.type) == wordCount) { "cannot assign a ${source.type} to a $type" }
            lowCopies.remove(register)
            moveValue(register, source.register, type)
        }

        override fun cast(type: String): ValueRef = cast(this, descriptorOf(type))
        override fun field(field: FieldTarget): ValueRef = readField(this, field.ref)
        override fun field(ref: FieldRef): ValueRef = readField(this, ref)
        override fun fieldOfType(type: String): ValueRef = readField(this, uniqueField(this.type, descriptorOf(type)))
        override fun set(field: FieldTarget, value: ValueRef) = writeField(this, field.ref, value.impl())
        override fun set(ref: FieldRef, value: ValueRef) = writeField(this, ref, value.impl())

        override fun call(method: ExtMethod, vararg args: ValueRef): ValueRef {
            require(!method.isStatic) { "$method is static; call it without a receiver" }
            return invoke(Opcode.INVOKE_VIRTUAL, method.ref, listOf(this) + args.map { it.impl() })
        }

        override fun call(method: MethodTarget, vararg args: ValueRef): ValueRef {
            require(!method.method.isStatic) { "${method.descriptor} is static; call it without a receiver" }
            return invoke(instanceInvokeKind(method), method.ref, listOf(this) + args.map { it.impl() })
        }

        override fun callVirtual(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef =
            invoke(Opcode.INVOKE_VIRTUAL, MethodRef(descriptorOf(owner), name, proto), listOf(this) + args.map { it.impl() })

        override fun callInterface(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef =
            invoke(Opcode.INVOKE_INTERFACE, MethodRef(descriptorOf(owner), name, proto), listOf(this) + args.map { it.impl() })

        override fun size(): ValueRef = invoke(Opcode.INVOKE_INTERFACE, MethodRef(Type.List, "size", "()I"), listOf(this))

        override fun get(index: ValueRef): ValueRef =
            invoke(Opcode.INVOKE_INTERFACE, MethodRef(Type.List, "get", "(I)Ljava/lang/Object;"), listOf(this, index.impl()))

        override fun plus(other: ValueRef): ValueRef = arithmetic(Opcode.ADD_INT, this, other.impl())
        override fun minus(other: ValueRef): ValueRef = arithmetic(Opcode.SUB_INT, this, other.impl())
    }

    internal fun ValueRef.impl(): Value {
        val value = this as? Value ?: error("ValueRef is only valid inside the code block it was created in")
        require(value.emitter === this@CodeEmitter) { "ValueRef belongs to a different code block" }
        return value
    }
}

private fun List<Int>.isConsecutive(): Boolean = indices.all { it == 0 || this[it] == this[it - 1] + 1 }

private fun virtualRegister(tempId: Int, offset: Int = 0): Int = -1 - (tempId * VIRTUAL_REGISTER_STRIDE + offset)
private fun isVirtualRegister(register: Int): Boolean = register < 0 && register != VOID_REGISTER
private fun virtualRegisterId(register: Int): Int = (-register - 1) / VIRTUAL_REGISTER_STRIDE
private fun virtualRegisterOffset(register: Int): Int = (-register - 1) % VIRTUAL_REGISTER_STRIDE

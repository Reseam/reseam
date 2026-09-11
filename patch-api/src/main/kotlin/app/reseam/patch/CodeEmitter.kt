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

internal class EntrySnapshot(val offset: Int, val register: Int, val type: String)

private class TempAllocation(val wordCount: Int, var constraint: RegisterConstraint, val entryOffset: Int? = null) {
    var baseRegister = -1
}

private class InvokeAllocation(val registers: List<Int>, val types: List<String>) {
    var scratch: Int? = null
    lateinit var operation: Op
}

private typealias Emit = (InstructionBuilder, resolve: (Int) -> Int) -> Unit

private class Op(
    val reads: List<Int> = emptyList(),
    val writes: List<Int> = emptyList(),
    val label: String? = null,
    val target: String? = null,
    val fallsThrough: Boolean = true,
    val emit: Emit,
)

private data class Lifetime(val first: Int, val last: Int)

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
    private val entrySnapshots: MutableMap<Int, EntrySnapshot>? = null,
) : CodeScope {
    private val ops = mutableListOf<Op>()
    private val usedRegisters = mutableSetOf<Int>()
    private var nextTempId = 0
    private var plannedLocalGrowth = 0
    private var maxOutRegisters = 0
    private var labelCounter = 0
    private val tempAllocations = mutableMapOf<Int, TempAllocation>()
    private val entryValues = mutableMapOf<Int, Value>()
    private val invokes = mutableListOf<InvokeAllocation>()
    /** Low-register copies of incoming registers, reused until the register is assigned. */
    private val lowCopies = mutableMapOf<Int, Value>()

    companion object {
        fun forInsertion(method: Method, index: Int, captures: List<Capture>, entrySnapshots: MutableMap<Int, EntrySnapshot>? = null) =
            CodeEmitter(method, index, replaceMode = false, captures = captures, entrySnapshots = entrySnapshots)

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
            return incomingValue(0, info.classDescriptor)
        }

    override fun param(index: Int): ValueRef {
        val params = method.parameterTypes
        require(index in params.indices) { "Parameter index $index out of bounds for ${info.descriptor}" }
        val offset = (if (isStaticMethod) 0 else 1) + params.take(index).sumOf(::registerWordCount)
        return incomingValue(offset, params[index])
    }

    private fun incomingValue(offset: Int, type: String): Value {
        val snapshots = entrySnapshots ?: return Value(incomingBase + offset, type)
        return entryValues.getOrPut(offset) {
            snapshots[offset]?.let { Value(it.register, type) } ?: run {
                val id = nextTempId++
                tempAllocations[id] = TempAllocation(registerWordCount(type), RegisterConstraint.ANY, offset)
                Value(virtualRegister(id), type)
            }
        }
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
        op(writes = listOf(dest)) { b, r -> b.constInt(byte(r(dest), "const"), value) }
        return Value(dest, Type.Int)
    }

    override fun long(value: Long): ValueRef {
        val dest = allocTemp(wordCount = 2)
        op(writes = listOf(dest)) { b, r -> b.constLong(byte(r(dest), "const-wide"), value) }
        return Value(dest, Type.Long)
    }

    override fun bool(value: Boolean): ValueRef {
        val dest = allocTemp()
        op(writes = listOf(dest)) { b, r -> b.constInt(byte(r(dest), "const"), if (value) 1 else 0) }
        return Value(dest, Type.Boolean)
    }

    override fun string(value: String): ValueRef {
        val dest = allocTemp()
        op(writes = listOf(dest)) { b, r -> b.constString(byte(r(dest), "const-string"), value) }
        return Value(dest, Type.String)
    }

    override val nullObject: ValueRef
        get() {
            val dest = allocTemp()
            op(writes = listOf(dest)) { b, r -> b.constInt(byte(r(dest), "const"), 0) }
            return Value(dest, Type.Object)
        }

    override fun enumValue(type: String, name: String): ValueRef {
        val owner = descriptorOf(type)
        return staticField(FieldRef(owner, name, owner))
    }

    override fun staticField(field: FieldTarget): ValueRef = staticField(field.ref)

    override fun staticField(field: FieldRef): ValueRef {
        val dest = allocTemp(registerWordCount(field.fieldType))
        op(writes = listOf(dest)) { b, r -> b.sgetTyped(byte(r(dest), "sget"), field) }
        return Value(dest, field.fieldType)
    }

    override fun newInstance(type: String, ctorProto: String, vararg args: ValueRef): ValueRef {
        val owner = descriptorOf(type)
        val dest = allocTemp()
        op(writes = listOf(dest)) { b, r -> b.newInstance(byte(r(dest), "new-instance"), owner) }
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
        return branch(block) { elseLabel -> op(reads = listOf(register), target = elseLabel) { b, r -> b.ifEqz(byte(r(register), "if-eqz"), elseLabel) } }
    }

    override fun whenFalse(value: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val register = value.impl().asByte().register
        return branch(block) { elseLabel -> op(reads = listOf(register), target = elseLabel) { b, r -> b.ifNez(byte(r(register), "if-nez"), elseLabel) } }
    }

    override fun whenNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise = whenFalse(value, block)

    override fun whenNotNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise = whenTrue(value, block)

    override fun whenEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val (a, c) = left.impl().asLow().register to right.impl().asLow().register
        return branch(block) { elseLabel -> op(reads = listOf(a, c), target = elseLabel) { b, r -> b.ifNe(low(r(a), "if-ne A"), low(r(c), "if-ne B"), elseLabel) } }
    }

    override fun whenNotEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val (a, c) = left.impl().asLow().register to right.impl().asLow().register
        return branch(block) { elseLabel -> op(reads = listOf(a, c), target = elseLabel) { b, r -> b.ifEq(low(r(a), "if-eq A"), low(r(c), "if-eq B"), elseLabel) } }
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
                ops.add(elseIndex, Op(target = endLabel, fallsThrough = false) { b, _ -> b.goto(endLabel) })
                this@CodeEmitter.block()
                label(endLabel)
            }
        }
    }

    override fun returnVoid() { op(fallsThrough = false) { b, _ -> b.returnVoid() } }

    override fun returnValue(value: ValueRef) {
        val impl = value.impl().asByte()
        val type = impl.type
        op(reads = listOf(impl.register), fallsThrough = false) { b, r ->
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
        layoutRegisters()
        val builder = InstructionBuilder()
        for (op in ops) op.emit(builder, ::resolveRegister)
        return builder.build()
    }

    internal fun invoke(opcode: Opcode, ref: MethodRef, args: List<Value>): ValueRef {
        val registers = args.flatMap { it.registerWords() }
        maxOutRegisters = maxOf(maxOutRegisters, registers.size)
        emitInvoke(opcode, ref, args, registers)
        val returnType = ref.returnType
        if (returnType == Type.Void) return Value(VOID_REGISTER, Type.Void)
        val dest = allocTemp(registerWordCount(returnType))
        op(writes = listOf(dest)) { b, r -> b.moveResultTyped(byte(r(dest), "move-result"), returnType) }
        return Value(dest, returnType)
    }

    private fun emitInvoke(opcode: Opcode, ref: MethodRef, args: List<Value>, registers: List<Int>) {
        val invoke = InvokeAllocation(registers, args.map { it.type })
        invokes += invoke
        invoke.operation = op(reads = registers) { b, r ->
            val resolved = registers.map(r)
            if (resolved.fitsInvoke()) {
                b.invoke(opcode, ref, resolved)
            } else {
                val range = opcode.rangeVariant ?: error("$opcode does not support invoke/range lowering")
                val scratch = invoke.scratch
                if (scratch == null) {
                    b.invokeRange(range, ref, resolved.first(), resolved.size)
                } else {
                    var word = 0
                    for (type in invoke.types) {
                        b.moveTyped(r(scratch) + word, resolved[word], type)
                        word += registerWordCount(type)
                    }
                    b.invokeRange(range, ref, r(scratch), resolved.size)
                }
            }
        }
    }

    internal fun readField(value: Value, field: FieldRef): ValueRef {
        val dest = allocTemp(registerWordCount(field.fieldType), RegisterConstraint.LOW)
        val obj = value.asLow()
        op(reads = listOf(obj.register), writes = listOf(dest)) { b, r -> b.igetTyped(low(r(dest), "iget A"), low(r(obj.register), "iget B"), field) }
        return Value(dest, field.fieldType)
    }

    internal fun writeField(value: Value, field: FieldRef, newValue: Value) {
        val src = newValue.asLow()
        val obj = value.asLow()
        op(reads = listOf(src.register, obj.register)) { b, r -> b.iputTyped(low(r(src.register), "iput A"), low(r(obj.register), "iput B"), field) }
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
        op(reads = listOf(a.register, c.register), writes = listOf(dest)) { b, r -> b.reg3(opcode, byte(r(dest), "binop A"), byte(r(a.register), "binop B"), byte(r(c.register), "binop C")) }
        return Value(dest, Type.Int)
    }

    internal fun cast(value: Value, type: String): ValueRef {
        val target = value.asByte()
        op(reads = listOf(target.register), writes = listOf(target.register)) { b, r -> b.checkCast(byte(r(target.register), "check-cast"), type) }
        return Value(target.register, type)
    }

    internal fun label(name: String) {
        lowCopies.clear()
        op(label = name) { b, _ -> b.label(name) }
    }

    internal fun goto(label: String) { op(target = label, fallsThrough = false) { b, _ -> b.goto(label) } }

    internal fun ifZero(value: Value, label: String) {
        val v = value.asByte()
        op(reads = listOf(v.register), target = label) { b, r -> b.ifEqz(byte(r(v.register), "if-eqz"), label) }
    }

    internal fun ifNonZero(value: Value, label: String) {
        val v = value.asByte()
        op(reads = listOf(v.register), target = label) { b, r -> b.ifNez(byte(r(v.register), "if-nez"), label) }
    }

    internal fun constZero(dest: Int, type: String) {
        op(writes = listOf(dest)) { b, r ->
            if (registerWordCount(type) == 2) b.constLong(byte(r(dest), "const-wide"), 0) else b.constInt(byte(r(dest), "const"), 0)
        }
    }

    internal fun instanceOf(value: Value, type: String): Value {
        val dest = allocTemp(constraint = RegisterConstraint.LOW)
        val ref = value.asLow()
        op(reads = listOf(ref.register), writes = listOf(dest)) { b, r -> b.instanceOf(low(r(dest), "instance-of A"), low(r(ref.register), "instance-of B"), type) }
        return Value(dest, Type.Boolean)
    }

    internal fun nextLabel(prefix: String): String = "${prefix}_${labelCounter++}"

    private fun op(
        reads: List<Int> = emptyList(),
        writes: List<Int> = emptyList(),
        label: String? = null,
        target: String? = null,
        fallsThrough: Boolean = true,
        emit: Emit,
    ): Op = Op(reads, writes, label, target, fallsThrough, emit).also { ops += it }

    internal fun allocTemp(
        wordCount: Int = 1,
        constraint: RegisterConstraint = RegisterConstraint.BYTE,
    ): Int {
        require(wordCount > 0)
        val tempId = nextTempId++
        tempAllocations[tempId] = TempAllocation(wordCount, constraint)
        return virtualRegister(tempId)
    }

    /** Allocate values after their uses and control-flow edges have been recorded. */
    private fun layoutRegisters() {
        val lifetimes = lifetimes()
        do {
            plannedLocalGrowth = 0
            usedRegisters.clear()
            for (allocation in tempAllocations.values) allocation.baseRegister = -1
            for (allocation in tempAllocations.values.filter { it.entryOffset != null }) {
                allocation.baseRegister = allocateLocal(allocation.wordCount, allocation.constraint)
            }
            val savedGrowth = plannedLocalGrowth
            val protected = usedRegisters.toMutableSet()
            for (capture in captures) protected += capture.register until capture.register + registerWordCount(capture.type)
            entrySnapshots?.values?.forEach { protected += it.register until it.register + registerWordCount(it.type) }
            protected += incomingBase until originalRegistersSize
            for (invoke in invokes) {
                invoke.scratch?.let {
                    val position = ops.indexOf(invoke.operation) * 2
                    lifetimes[virtualRegisterId(it)] = Lifetime(position, position + 1)
                }
            }
            val active = mutableListOf<Pair<TempAllocation, Int>>()
            val allocations = tempAllocations.entries.filter { it.value.entryOffset == null }
                .sortedWith(compareBy({ lifetimes[it.key]?.first ?: 0 }, { it.value.constraint.maxRegister }, { it.key }))
            for ((id, allocation) in allocations) {
                val lifetime = lifetimes[id] ?: Lifetime(0, 0)
                active.removeAll { (_, last) -> last < lifetime.first }
                usedRegisters.clear()
                usedRegisters += protected
                for ((slot, _) in active) usedRegisters += slot.baseRegister until slot.baseRegister + slot.wordCount
                val occupied = active.flatMap { (slot, _) ->
                    (slot.baseRegister until slot.baseRegister + slot.wordCount).toList()
                }.toSet()
                allocation.baseRegister = allocateTemp(allocation, savedGrowth, occupied)
                active += allocation to lifetime.last
            }

            // New staging spans participate in the same lifetime allocation.
            // Each invoke can add at most one; rerun layout until encodings fit.
            var allocated = false
            for (invoke in invokes) {
                val registers = invoke.registers.map(::resolveRegister)
                if (invoke.scratch == null && !registers.fitsInvoke() && !registers.isConsecutive()) {
                    invoke.scratch = allocTemp(registers.size, RegisterConstraint.ANY)
                    allocated = true
                }
            }
        } while (allocated)
        for ((offset, value) in entryValues) {
            entrySnapshots!![offset] = EntrySnapshot(offset, resolveRegister(value.register), value.type)
        }
    }

    private fun lifetimes(): MutableMap<Int, Lifetime> {
        fun virtualIds(registers: List<Int>) = registers.filter(::isVirtualRegister).map(::virtualRegisterId).toSet()
        val reads = ops.map { virtualIds(it.reads) }
        val writes = ops.map { virtualIds(it.writes) }
        val labels = ops.mapIndexedNotNull { index, op -> op.label?.let { it to index } }.toMap()
        val successors = ops.mapIndexed { index, op ->
            buildList {
                if (op.fallsThrough && index + 1 < ops.size) add(index + 1)
                op.target?.let { add(labels.getValue(it)) }
            }
        }
        val liveIn = List(ops.size) { mutableSetOf<Int>() }
        val liveOut = List(ops.size) { mutableSetOf<Int>() }
        do {
            var changed = false
            for (index in ops.indices.reversed()) {
                val out = successors[index].flatMap { liveIn[it] }.toSet()
                val input = reads[index] + (out - writes[index])
                if (input != liveIn[index] || out != liveOut[index]) {
                    liveIn[index].clear()
                    liveIn[index] += input
                    liveOut[index].clear()
                    liveOut[index] += out
                    changed = true
                }
            }
        } while (changed)
        val lifetimes = mutableMapOf<Int, Lifetime>()
        fun touch(ids: Set<Int>, position: Int) {
            for (id in ids) {
                val previous = lifetimes[id]
                lifetimes[id] = Lifetime(minOf(previous?.first ?: position, position), maxOf(previous?.last ?: position, position))
            }
        }
        for (index in ops.indices) {
            touch(liveIn[index] + reads[index], index * 2)
            touch(liveOut[index] + writes[index], index * 2 + 1)
        }
        return lifetimes
    }

    private fun allocateTemp(allocation: TempAllocation, savedGrowth: Int, occupied: Set<Int>): Int {
        val words = allocation.wordCount
        fun available(first: Int, end: Int): Int? = (first..end - words).firstOrNull { base ->
            base + words - 1 <= allocation.constraint.maxRegister && (base until base + words).none { it in occupied }
        }
        if (replaceMode) {
            return requireNotNull(available(0, REPLACE_LOCAL_BUDGET)) {
                "Code exceeded the $REPLACE_LOCAL_BUDGET local registers a replaced body gets in ${info.descriptor}"
            }
        }
        val registers = method.findContiguousFreeRegisters(insertIndex ?: 0, words, usedRegisters.toList())
        if (registers.size == words && registers.last() <= allocation.constraint.maxRegister) return registers.first()
        available(incomingBase + savedGrowth, incomingBase + plannedLocalGrowth)?.let { return it }
        return allocateLocal(words, allocation.constraint)
    }

    private fun allocateLocal(wordCount: Int, constraint: RegisterConstraint): Int {
        val index = insertIndex ?: 0
        val grownBase = incomingBase + plannedLocalGrowth
        val grownLast = grownBase + wordCount - 1
        val newRegistersSize = originalRegistersSize + plannedLocalGrowth + wordCount
        require(grownLast <= constraint.maxRegister && newRegistersSize <= UShort.MAX_VALUE.toInt()) {
            "Cannot allocate $wordCount ${constraint.name.lowercase()} scratch register(s) at ${info.descriptor}[$index]; " +
                "plannedLocalGrowth=$plannedLocalGrowth, registersSize=$originalRegistersSize, insSize=${method.insSize}"
        }
        plannedLocalGrowth += wordCount
        usedRegisters += (grownBase..grownLast)
        return grownBase
    }

    internal fun moveValue(dest: Int, src: Int, type: String) {
        op(reads = listOf(src), writes = listOf(dest)) { b, r -> b.moveTyped(r(dest), r(src), type) }
    }

    internal fun registerWords(register: Int, wordCount: Int): List<Int> =
        if (isVirtualRegister(register)) {
            (0 until wordCount).map { virtualRegister(virtualRegisterId(register), it) }
        } else {
            (0 until wordCount).map { register + it }
        }

    private fun registerFits(register: Int, wordCount: Int, constraint: RegisterConstraint): Boolean {
        if (register == VOID_REGISTER) return false
        if (isVirtualRegister(register)) {
            return tempAllocations.getValue(virtualRegisterId(register)).constraint.maxRegister <= constraint.maxRegister
        }
        return register >= 0 && register + wordCount - 1 <= constraint.maxRegister && !isShiftedPhysical(register)
    }

    private fun isShiftedPhysical(register: Int): Boolean = !replaceMode && register >= incomingBase

    private fun resolveRegister(register: Int): Int {
        require(register != VOID_REGISTER) { "a void value has no register" }
        if (isVirtualRegister(register)) {
            val allocation = tempAllocations[virtualRegisterId(register)] ?: error("unallocated virtual register $register")
            check(allocation.baseRegister >= 0) { "virtual register $register has not been laid out" }
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
            if (registerFits(register, wordCount, constraint)) return this
            if (isVirtualRegister(register)) {
                val allocation = tempAllocations.getValue(virtualRegisterId(register))
                if (allocation.entryOffset == null) {
                    allocation.constraint = constraint
                    return this
                }
            }
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
private fun List<Int>.fitsInvoke(): Boolean = size <= 5 && all { it in 0..15 }

private fun virtualRegister(tempId: Int, offset: Int = 0): Int = -1 - (tempId * VIRTUAL_REGISTER_STRIDE + offset)
private fun isVirtualRegister(register: Int): Boolean = register < 0 && register != VOID_REGISTER
private fun virtualRegisterId(register: Int): Int = (-register - 1) / VIRTUAL_REGISTER_STRIDE
private fun virtualRegisterOffset(register: Int): Int = (-register - 1) % VIRTUAL_REGISTER_STRIDE

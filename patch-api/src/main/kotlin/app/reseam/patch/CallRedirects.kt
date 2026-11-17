// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.arguments
import app.reseam.patch.dex.arrayReads
import app.reseam.patch.dex.arrayWrites
import app.reseam.patch.dex.buildInstructions
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.invokeRegisters
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.regA
import app.reseam.patch.dex.returnType
import app.reseam.patch.emit.Access
import app.reseam.patch.emit.EmittedCode
import app.reseam.patch.emit.ValueType
import app.reseam.patch.emit.builtBy
import app.reseam.patch.emit.insertCode
import app.reseam.patch.emit.invoke
import app.reseam.patch.emit.requireArgumentTypes
import app.reseam.patch.native.isAssignable
import app.reseam.patch.points.ArgumentOrigins
import app.reseam.patch.types.CodePlacement
import app.reseam.patch.types.Instruction

/**
 * Redirect this invoke to a static extension, passing the receiver first for instance calls. An
 * array read becomes `target(array, index)`, whose result the app takes as the element, and a write
 * becomes `target(array, index, value)`.
 */
fun PointTarget.redirectTo(target: ExtMethod) {
    val opcode = resolved.instruction.opcode
    if (opcode != null && (opcode in arrayReads || opcode in arrayWrites))
        redirectArrayAccess(target)
    else prepareRedirect(target).apply()
}

internal fun List<PointTarget>.redirectTo(target: ExtMethod): Int {
    val edits = map { it.prepareRedirect(target) }
    edits.forEach { it.apply() }
    return size
}

private class CallRedirect(val point: ResolvedPoint, val replacement: Instruction) {
    fun apply() = point.method.replaceInstruction(point.index, replacement)
}

private fun PointTarget.prepareRedirect(target: ExtMethod): CallRedirect {
    val point = resolved
    val insns = point.method.instructions
    val call = insns[point.index]
    val location = "$label: ${point.method.descriptor}[${point.index}]"
    require(call.opcode in redirectableInvokes) {
        "$location: redirectTo does not support ${call.opcode}"
    }
    val from = requireNotNull(call.methodRef) { "$location: invoke has no method reference" }
    require(from.name != "<init>") { "$location: constructors cannot be redirected" }
    requireStaticTarget(target, location)
    Access(point.method.owner).requireMethod(target.ref)
    val actual = call.arguments(location).map { ValueType.Known(it.type) }
    val expected = target.ref.parameterTypes
    requireArgumentTypes(actual, expected, "$location redirect ${from.descriptor} to $target")
    val registers = call.invokeRegisters.orEmpty()
    if (insns.getOrNull(point.index + 1)?.opcode?.isMoveResult == true) {
        require(isAssignable(target.ref.returnType, from.returnType)) {
            "$location: result of ${from.descriptor} is consumed; $target returns an incompatible type"
        }
    }
    // The registers and word count are unchanged. The existing invoke lowering
    // chooses 35c/range; replacement retains incoming branches and tracked anchors.
    val replacement = buildInstructions {
        invokeStatic(target.owner, target.name, target.proto, *registers.toIntArray())
    }
        .single()
    return CallRedirect(point, replacement)
}

// An array access has no invoke to swap in place, and its registers may not fit one, so the call
// goes in as code before it and the access is removed.
private fun PointTarget.redirectArrayAccess(target: ExtMethod) {
    val point = resolved
    val method = point.method
    val insns = method.instructions
    val access = insns[point.index]
    val location = "$label: ${method.descriptor}[${point.index}]"
    requireStaticTarget(target, location)
    Access(method.owner).requireMethod(target.ref)
    val origins = ArgumentOrigins(method)
    val types =
        checkNotNull(origins.operandTypes(insns, point.index)).map {
            it ?: error("$location: the array's type is unknown here, so $target cannot be checked")
        }
    requireArgumentTypes(
        types.map { ValueType.Known(it) },
        target.ref.parameterTypes,
        "$location redirect ${access.opcode} to $target",
    )
    val element = types[0].substring(1)
    val result =
        if (access.opcode?.let { it in arrayReads } == true) {
            require(isAssignable(target.ref.returnType, element)) {
                "$location: the app takes the element as $element; $target returns an incompatible type"
            }
            Capture("redirectResult", element, access.regA!!)
        } else null
    val operands = types.mapIndexed { i, type ->
        Capture(
            "redirect$i",
            type,
            origins.atEntry(insns, point.index, i, point.anchor.head, label),
        )
    }
    method.insertCode(
        CodePlacement.OnEveryPath(point.anchor.head.toUInt()),
        {
            val emitter = (this as EmittedCode).emitter
            val value =
                emitter.invoke(
                    Opcode.INVOKE_STATIC,
                    target.ref,
                    operands.map { capture(it.name).builtBy(emitter) },
                )
            result?.let { capture(it.name).assign(value) }
        },
        point.captures + operands + listOfNotNull(result),
    )
    method.removeInstructions(point.index, 1)
}

private fun requireStaticTarget(target: ExtMethod, location: String) =
    require(target.isStatic && target.target.method.isStatic) {
        "$location: redirect target $target must be static"
    }

internal val redirectableInvokes =
    setOf(
        Opcode.INVOKE_STATIC,
        Opcode.INVOKE_STATIC_RANGE,
        Opcode.INVOKE_VIRTUAL,
        Opcode.INVOKE_VIRTUAL_RANGE,
        Opcode.INVOKE_INTERFACE,
        Opcode.INVOKE_INTERFACE_RANGE,
    )

// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.buildInstructions
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.invokeRegisters
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.types.Instruction

/** Redirect this invoke to a static extension, passing the receiver first for instance calls. */
fun PointTarget.redirectTo(target: ExtMethod) = prepareRedirect(target).apply()

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
    require(target.isStatic && target.target.method.isStatic) {
        "$location: redirect target $target must be static"
    }
    Access(point.method.owner).requireMethod(target.ref)
    val actual = call.arguments(location).map { ValueType.Known(it.type) }
    val expected = target.ref.parameterTypes
    requireArgumentTypes(actual, expected, "$location redirect ${from.descriptor} to $target")
    val registers = call.invokeRegisters.orEmpty()
    if (insns.getOrNull(point.index + 1)?.opcode?.isMoveResult == true) {
        require(isAssignableType(target.ref.returnType, from.returnType)) {
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

internal val redirectableInvokes =
    setOf(
        Opcode.INVOKE_STATIC,
        Opcode.INVOKE_STATIC_RANGE,
        Opcode.INVOKE_VIRTUAL,
        Opcode.INVOKE_VIRTUAL_RANGE,
        Opcode.INVOKE_INTERFACE,
        Opcode.INVOKE_INTERFACE_RANGE,
    )

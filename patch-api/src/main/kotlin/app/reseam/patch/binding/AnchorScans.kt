// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.binding

import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.returnType
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodRef

/** The first `iget-object` at [indices] reading a field of [owner], or of any class when null. */
internal fun List<Instruction>.objectRead(
    indices: IntProgression,
    owner: String?,
): IndexedValue<FieldRef>? = indices.firstNotNullOfOrNull { i ->
    this[i]
        .takeIf { it.opcode == Opcode.IGET_OBJECT }
        ?.fieldRef
        ?.takeIf { owner == null || it.definingClass == owner }
        ?.let { IndexedValue(i, it) }
}

/**
 * The first `invoke-interface` at [indices] of a method of [owner] returning [returnType] when
 * given, and an object when [returningObject].
 */
internal fun List<Instruction>.interfaceCall(
    indices: IntProgression,
    owner: String,
    returnType: String?,
    returningObject: Boolean,
): IndexedValue<MethodRef>? = indices.firstNotNullOfOrNull { i ->
    this[i]
        .takeIf {
            it.opcode == Opcode.INVOKE_INTERFACE || it.opcode == Opcode.INVOKE_INTERFACE_RANGE
        }
        ?.methodRef
        ?.takeIf {
            it.definingClass == owner &&
                (returnType == null || it.returnType == returnType) &&
                (!returningObject || isReferenceType(it.returnType))
        }
        ?.let { IndexedValue(i, it) }
}

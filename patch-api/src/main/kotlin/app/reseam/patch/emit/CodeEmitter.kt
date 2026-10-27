// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.Capture
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.method
import app.reseam.patch.run.ActiveRuntime

internal class CodeEmitter
private constructor(
    internal val method: Method,
    internal val insertIndex: Int?,
    internal val mode: EmissionMode,
    internal val captures: List<Capture>,
    internal val returned: Capture? = null,
    internal val entrySnapshots: MutableMap<Int, EntrySnapshot>? = null,
) {
    internal val ops = mutableListOf<Op>()
    internal val usedRegisters = mutableSetOf<Int>()
    internal var nextTempId = 0
    internal var plannedLocalGrowth = 0
    internal var replacementLocals = MIN_REPLACEMENT_LOCALS
    internal var maxOutRegisters = 0
    internal var labelCounter = 0
    internal val tempAllocations = mutableMapOf<TemporaryId, TempAllocation>()
    internal val entryValues = mutableMapOf<Int, Value>()
    internal val invokes = mutableListOf<InvokeAllocation>()
    internal val lowCopies = mutableMapOf<Register, Value>()

    companion object {
        fun forInsertion(
            method: Method,
            index: Int,
            captures: List<Capture>,
            returned: Capture? = null,
            entrySnapshots: MutableMap<Int, EntrySnapshot>? = null,
        ) =
            CodeEmitter(
                method,
                index,
                mode = EmissionMode.INSERTION,
                captures = captures,
                returned = returned,
                entrySnapshots = entrySnapshots,
            )

        fun forReplacement(method: Method) =
            CodeEmitter(
                method,
                insertIndex = null,
                mode = EmissionMode.REPLACEMENT,
                captures = emptyList(),
            )
    }

    internal val info = method.info
    internal val access = Access(info.classDescriptor)
    internal val reservedLocals =
        if (mode == EmissionMode.REPLACEMENT) emptyList()
        else ActiveRuntime.current.edits.reserved(method)
    internal val isStaticMethod = AccessFlags.STATIC.isSet(info.accessFlags)
    internal val incomingWords =
        if (mode == EmissionMode.REPLACEMENT)
            method.parameterTypes.sumOf(::registerWordCount) + if (isStaticMethod) 0 else 1
        else method.insSize
    internal val originalRegistersSize =
        if (mode == EmissionMode.REPLACEMENT) MIN_REPLACEMENT_LOCALS + incomingWords
        else method.registersSize
    internal val incomingBase =
        if (mode == EmissionMode.REPLACEMENT) MIN_REPLACEMENT_LOCALS
        else method.registersSize - incomingWords

    val code = EmittedCode(this)
}

// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

class PatchRuntime {
    val manifest: ManifestScope = ManifestScope()
    val resources: ResourceScope = ResourceScope()
    val bytecode: BytecodeScope = BytecodeScope()
    val files: FileScope = FileScope()
    val options: RuntimeOptions = RuntimeOptions()
    val log: PatchLogger = PatchLogger()

    internal val apiState: PatchApiState by lazy { PatchApiState(this) }
}

class BytecodeScope internal constructor() {
    val classes: List<DexClass>
        get() = getAllClasses().map { DexClass(it.toUInt()) }

    fun findClass(descriptor: String): DexClass? =
        app.reseam.patch.findClass(descriptor)?.let { DexClass(it) }
}

class PatchLogger internal constructor() {
    fun info(message: String) = logInfo(message)
    fun warn(message: String) = logWarn(message)
    fun debug(message: String) = logDebug(message)
}

package dev.stitch.patch

class PatchRuntime {
    val manifest: ManifestScope = ManifestScope()
    val resources: ResourceScope = ResourceScope()
    val bytecode: BytecodeScope = BytecodeScope()
    val files: FileScope = FileScope()
    val options: RuntimeOptions = RuntimeOptions()
    val log: PatchLogger = PatchLogger()
}

class BytecodeScope internal constructor() {
    val classes: List<DexClass>
        get() = getAllClasses().map { DexClass(it.toUInt()) }

    fun findClass(descriptor: String): DexClass? =
        dev.stitch.patch.findClass(descriptor)?.let { DexClass(it) }

    fun fingerprint(block: FingerprintBuilder.() -> Unit): Fingerprint =
        dev.stitch.patch.fingerprint(block)
}

class PatchLogger internal constructor() {
    fun info(message: String) = logInfo(message)
    fun warn(message: String) = logWarn(message)
    fun debug(message: String) = logDebug(message)
}

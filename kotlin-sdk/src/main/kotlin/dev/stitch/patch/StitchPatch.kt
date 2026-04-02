package dev.stitch.patch

class PatchException(message: String) : Exception(message)

enum class PatchOptionType {
    STRING,
    BOOL,
    INT,
    FLOAT,
    STRING_LIST,
    PATH,
}

data class PatchOption(
    val key: String,
    val title: String = key,
    val description: String = "",
    val type: PatchOptionType,
    val defaultString: String? = null,
    val defaultBool: Boolean? = null,
    val defaultInt: Long? = null,
    val defaultFloat: Double? = null,
    val defaultStringList: List<String>? = null,
    val validValues: List<String>? = null,
    val required: Boolean = false,
)

interface StitchPatch {
    val name: String
    val description: String get() = ""
    val dependencies: List<String> get() = emptyList()
    val compatibleWith: List<CompatiblePackage> get() = emptyList()
    // This controls default selection only. Callers can still explicitly enable or disable patches.
    val enabled: Boolean get() = true
    // Declarations are metadata; execution-time values are supplied by the host runtime.
    val options: List<PatchOption> get() = emptyList()
    val extensionDex: List<String> get() = emptyList()

    fun execute()
    fun afterDependents() {}
}

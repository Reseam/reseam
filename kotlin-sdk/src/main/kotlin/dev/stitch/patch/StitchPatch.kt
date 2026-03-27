package dev.stitch.patch

interface StitchPatch {
    val name: String
    val description: String get() = ""
    val dependencies: List<String> get() = emptyList()
    val compatibleWith: List<String> get() = emptyList()
    val enabled: Boolean get() = true

    fun execute()
}

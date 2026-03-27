package dev.stitch.patch

data class CompatiblePackage(
    val name: String,
    val versions: List<String> = emptyList(),
)

fun compatibleWith(name: String, vararg versions: String) =
    CompatiblePackage(name, versions.toList())

fun patch(
    name: String,
    description: String = "",
    compatibleWith: List<CompatiblePackage> = emptyList(),
    enabledByDefault: Boolean = true,
    dependsOn: List<Any> = emptyList(),
    block: PatchBuilder.() -> Unit,
): StitchPatch {
    val resolvedDeps = dependsOn.map { dep ->
        when (dep) {
            is String -> dep
            is StitchPatch -> dep.name
            else -> error("dependsOn accepts String or StitchPatch, got: ${dep::class}")
        }
    }
    return PatchBuilder(name, description, compatibleWith, enabledByDefault, resolvedDeps)
        .apply(block).build()
}

class PatchBuilder(
    val name: String,
    val description: String,
    val compatibleWith: List<CompatiblePackage>,
    val enabledByDefault: Boolean,
    val dependsOn: List<String>,
) {
    private var executeBlock: (() -> Unit)? = null

    fun execute(block: () -> Unit) {
        this.executeBlock = block
    }

    internal fun build(): StitchPatch {
        val p = this
        val exec = executeBlock
        return object : StitchPatch {
            override val name = p.name
            override val description = p.description
            override val dependencies = p.dependsOn
            override val compatibleWith = p.compatibleWith.map { it.name }
            override val enabled = p.enabledByDefault
            override fun execute() {
                exec?.invoke()
            }
        }
    }
}


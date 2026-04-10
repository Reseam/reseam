package dev.stitch.patch

typealias ClassDef = DexClass

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
    options: List<PatchOption> = emptyList(),
    block: PatchBuilder.() -> Unit,
): StitchPatch {
    val resolvedDeps = dependsOn.map { dep ->
        when (dep) {
            is String -> dep
            is StitchPatch -> dep.name
            else -> error("dependsOn accepts String or StitchPatch, got: ${dep::class}")
        }
    }
    return PatchBuilder(name, description, compatibleWith, enabledByDefault, resolvedDeps, options)
        .apply(block)
        .build()
}

class PatchBuilder(
    val name: String,
    val description: String,
    private val initCompatibleWith: List<CompatiblePackage>,
    val enabledByDefault: Boolean,
    private val initDependsOn: List<String>,
    private val initOptions: List<PatchOption>,
) {
    private var executeBlock: ((PatchRuntime) -> Unit)? = null
    private var finalizeBlock: ((PatchRuntime) -> Unit)? = null
    private val extraDependsOn = mutableListOf<String>()
    private val extraCompatibleWith = mutableListOf<CompatiblePackage>()
    private val extraOptions = mutableListOf<PatchOption>()
    private val extensionDexPaths = mutableListOf<String>()

    fun dependsOn(vararg deps: Any) {
        for (dep in deps) {
            extraDependsOn.add(
                when (dep) {
                    is String -> dep
                    is StitchPatch -> dep.name
                    else -> error("dependsOn accepts String or StitchPatch, got: ${dep::class}")
                }
            )
        }
    }

    fun compatibleWith(name: String, vararg versions: String) {
        extraCompatibleWith.add(CompatiblePackage(name, versions.toList()))
    }

    fun option(declaration: PatchOption) {
        extraOptions.add(declaration)
    }

    fun extendWith(vararg paths: String) {
        extensionDexPaths.addAll(paths)
    }

    fun execute(block: (PatchRuntime) -> Unit) {
        executeBlock = block
    }

    fun afterDependents(block: (PatchRuntime) -> Unit) {
        finalizeBlock = block
    }

    internal fun build(): StitchPatch {
        val allCompat = initCompatibleWith + extraCompatibleWith
        val allDeps = initDependsOn + extraDependsOn
        val allOptions = initOptions + extraOptions
        val allExtensions = extensionDexPaths.toList()
        val builder = this
        val exec = executeBlock
        val fin = finalizeBlock
        return object : StitchPatch {
            override val name = builder.name
            override val description = builder.description
            override val dependencies = allDeps
            override val compatibleWith = allCompat
            override val enabled = builder.enabledByDefault
            override val options = allOptions
            override val extensionDex = allExtensions

            override fun execute(ctx: PatchRuntime) {
                exec?.invoke(ctx)
            }

            override fun afterDependents(ctx: PatchRuntime) {
                fin?.invoke(ctx)
            }
        }
    }
}

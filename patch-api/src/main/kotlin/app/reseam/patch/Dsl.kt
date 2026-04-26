// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.settings.SettingsSection
import app.reseam.patch.settings.registerPatchSettings

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
    settingsHost: Any? = null,
    settings: List<SettingsSection> = emptyList(),
    block: PatchBuilder.() -> Unit,
): ReseamPatch {
    val resolvedDeps = dependsOn.map { dep ->
        when (dep) {
            is String -> dep
            is ReseamPatch -> dep.name
            else -> error("dependsOn accepts String or ReseamPatch, got: ${dep::class}")
        }
    }
    val resolvedSettingsHost = when (settingsHost) {
        null -> null
        is String -> settingsHost
        is ReseamPatch -> settingsHost.name
        else -> error("settingsHost accepts String or ReseamPatch, got: ${settingsHost::class}")
    }
    return PatchBuilder(
        name, description, compatibleWith, enabledByDefault,
        resolvedDeps, options, resolvedSettingsHost, settings,
    )
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
    private val initSettingsHost: String?,
    private val initSettings: List<SettingsSection>,
) {
    private var executeBlock: ((PatchRuntime) -> Unit)? = null
    private var finalizeBlock: ((PatchRuntime) -> Unit)? = null
    private val extraDependsOn = mutableListOf<String>()
    private val extraCompatibleWith = mutableListOf<CompatiblePackage>()
    private val extraOptions = mutableListOf<PatchOption>()
    private val extensionDexPaths = mutableListOf<String>()
    private val extraSettings = mutableListOf<SettingsSection>()
    private var settingsHostName = initSettingsHost

    fun dependsOn(vararg deps: Any) {
        for (dep in deps) {
            extraDependsOn.add(
                when (dep) {
                    is String -> dep
                    is ReseamPatch -> dep.name
                    else -> error("dependsOn accepts String or ReseamPatch, got: ${dep::class}")
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

    fun settings(vararg sections: SettingsSection) {
        extraSettings.addAll(sections)
    }

    fun settingsHost(host: Any) {
        settingsHostName = when (host) {
            is String -> host
            is ReseamPatch -> host.name
            else -> error("settingsHost accepts String or ReseamPatch, got: ${host::class}")
        }
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

    internal fun build(): ReseamPatch {
        val allCompat = initCompatibleWith + extraCompatibleWith
        val allDeps = initDependsOn + extraDependsOn
        val allOptions = initOptions + extraOptions
        val allExtensions = extensionDexPaths.toList()
        val allSettings = initSettings + extraSettings
        val allSettingsHost = settingsHostName
        val builder = this
        val exec = executeBlock
        val fin = finalizeBlock
        return object : ReseamPatch {
            override val name = builder.name
            override val description = builder.description
            override val dependencies = allDeps
            override val compatibleWith = allCompat
            override val enabled = builder.enabledByDefault
            override val options = allOptions
            override val extensionDex = allExtensions
            override val settingsHost = allSettingsHost
            override val settings = allSettings

            override fun execute(ctx: PatchRuntime) {
                registerPatchSettings(allSettingsHost, name, allSettings)
                exec?.invoke(ctx)
            }

            override fun afterDependents(ctx: PatchRuntime) {
                fin?.invoke(ctx)
            }
        }
    }
}

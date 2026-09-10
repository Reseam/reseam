// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.settings.SettingsHost
import app.reseam.patch.settings.SettingsSection

interface ReseamPatch {
    /** Shown to users. A patch without one is internal: it only runs as a dependency and is never listed. */
    val name: String?
    val hidden: Boolean get() = name == null
    val description: String get() = ""
    val dependencies: List<ReseamPatch> get() = emptyList()
    val compatibleWith: List<CompatiblePackage> get() = emptyList()
    val enabled: Boolean get() = !hidden
    val options: List<Option<*>> get() = emptyList()

    fun execute(ctx: PatchRuntime)
    fun afterDependents(ctx: PatchRuntime) {}
}

data class CompatiblePackage(
    val name: String,
    /** Empty means every version. */
    val versions: List<String> = emptyList(),
)

operator fun String.invoke(vararg versions: String) = CompatiblePackage(this, versions.toList())

fun patch(name: String, block: PatchBuilder.() -> Unit): ReseamPatch =
    PatchBuilder(name).apply(block).build()

fun patch(block: PatchBuilder.() -> Unit): ReseamPatch =
    PatchBuilder(null).apply(block).build()

abstract class PatchDeclaration internal constructor() {
    internal val compatibility = mutableListOf<CompatiblePackage>()
    internal val dependencies = mutableListOf<ReseamPatch>()

    fun compatibleWith(vararg packageNames: String) {
        compatibility += packageNames.map { CompatiblePackage(it) }
    }

    fun compatibleWith(vararg packages: CompatiblePackage) {
        compatibility += packages
    }

    fun dependsOn(vararg patches: ReseamPatch) {
        dependencies += patches
    }
}

class PatchBuilder internal constructor(private val name: String?) : PatchDeclaration() {
    private var description = ""
    private var hidden = name == null
    private var enabledByDefault = true
    private val options = mutableListOf<Option<*>>()
    private var settingsHost: SettingsHost? = null
    private val settings = mutableListOf<SettingsSection>()
    private var executeBlock: (PatchRuntime.() -> Unit)? = null
    private var afterDependentsBlock: (PatchRuntime.() -> Unit)? = null

    fun description(text: String) {
        description = text.trimIndent()
    }

    fun enabledByDefault(value: Boolean) {
        enabledByDefault = value
    }

    /** Keeps a named patch off the user-facing lists. */
    fun hidden() {
        hidden = true
    }

    fun stringOption(key: String, title: String = key, description: String = "", default: String? = null, validValues: List<String>? = null, required: Boolean = false) =
        StringOption(key, title, description, required, default, validValues).also { options += it }

    fun boolOption(key: String, title: String = key, description: String = "", default: Boolean? = null, required: Boolean = false) =
        BoolOption(key, title, description, required, default).also { options += it }

    fun intOption(key: String, title: String = key, description: String = "", default: Long? = null, required: Boolean = false) =
        IntOption(key, title, description, required, default).also { options += it }

    fun floatOption(key: String, title: String = key, description: String = "", default: Double? = null, required: Boolean = false) =
        FloatOption(key, title, description, required, default).also { options += it }

    fun stringListOption(key: String, title: String = key, description: String = "", default: List<String>? = null, required: Boolean = false) =
        StringListOption(key, title, description, required, default).also { options += it }

    fun pathOption(key: String, title: String = key, description: String = "", required: Boolean = false) =
        PathOption(key, title, description, required).also { options += it }

    /** Registers settings with `host`, which becomes a dependency. */
    fun settings(host: SettingsHost, vararg sections: SettingsSection) {
        settingsHost = host
        settings += sections
        if (host !in dependencies) dependencies += host
    }

    fun execute(block: PatchRuntime.() -> Unit) {
        executeBlock = block
    }

    fun afterDependents(block: PatchRuntime.() -> Unit) {
        afterDependentsBlock = block
    }

    internal fun build(): ReseamPatch {
        val builder = this
        val host = settingsHost
        val sections = settings.toList()
        return object : ReseamPatch {
            override val name = builder.name
            override val hidden = builder.hidden
            override val description = builder.description
            override val dependencies = builder.dependencies.toList()
            override val compatibleWith = builder.compatibility.toList()
            override val enabled = !builder.hidden && builder.enabledByDefault
            override val options = builder.options.toList()

            override fun execute(ctx: PatchRuntime) {
                host?.register(this, sections)
                ActiveRuntime.run(ctx) { executeBlock?.invoke(ctx) }
            }

            override fun afterDependents(ctx: PatchRuntime) {
                ActiveRuntime.run(ctx) { afterDependentsBlock?.invoke(ctx) }
            }

            override fun toString() = name ?: "internal patch"
        }
    }
}

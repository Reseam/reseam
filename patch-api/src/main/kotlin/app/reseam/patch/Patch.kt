// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.settings.SettingsContribution
import app.reseam.patch.settings.SettingsHost
import app.reseam.patch.settings.SettingsSection
import app.reseam.patch.settings.StringRef

operator fun String.invoke(vararg versions: String) = CompatiblePackage(this, versions.toList())

/** A version known by its name and code; an APK matches when either one does. */
operator fun String.invoke(versionName: String, versionCode: Int) =
    CompatiblePackage(this, listOf(versionName), listOf(versionCode))

/**
 * A patch users see as [name], a string of the module's `src/main/strings` that the bundle's
 * catalog carries in every language the module translates it to.
 */
fun patch(name: StringRef, block: PatchBuilder.() -> Unit): ReseamPatch =
    PatchBuilder(name).apply(block).build()

fun patch(block: PatchBuilder.() -> Unit): ReseamPatch = PatchBuilder(null).apply(block).build()

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

class PatchBuilder internal constructor(private val name: StringRef?) : PatchDeclaration() {
    private var description: StringRef? = null
    private var hidden = name == null
    private var enabledByDefault: Boolean? = null
    private val options = mutableListOf<Option<*>>()
    private val settings =
        mutableListOf<Pair<SettingsHost, PatchRuntime.() -> List<SettingsContribution>>>()
    private var executeBlock: (PatchRuntime.() -> Unit)? = null
    private var afterDependentsBlock: (PatchRuntime.() -> Unit)? = null
    private var afterWriteBlock: (OutputRuntime.() -> Unit)? = null

    fun description(text: StringRef) {
        description = text
    }

    fun enabledByDefault(value: Boolean) {
        enabledByDefault = value
    }

    /** Keeps a named patch off the user-facing lists. */
    fun hidden() {
        hidden = true
    }

    /**
     * Offers [presets], labelled values, beside free entry. Without [allowCustom] only the presets
     * are accepted. [validValues] is the unlabelled form of that.
     */
    fun stringOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: String? = null,
        validValues: List<String>? = null,
        required: Boolean = false,
        presets: List<OptionPreset<String>> = emptyList(),
        allowCustom: Boolean = true,
    ): StringOption {
        require(validValues == null || presets.isEmpty()) {
            "Option '$key' declares both validValues and presets; use presets with allowCustom = false"
        }
        val allowed = validValues ?: presets.takeUnless { allowCustom }?.map { it.value }
        val checked = presets.checked(key, allowCustom)
        return declare(
            StringOption(key, title, description, required, default, allowed, checked, allowCustom)
        )
    }

    @Deprecated(
        "Bundles built against 1.0 call stringOption without presets",
        level = DeprecationLevel.HIDDEN,
    )
    fun stringOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: String? = null,
        validValues: List<String>? = null,
        required: Boolean = false,
    ) = stringOption(key, title, description, default, validValues, required, emptyList(), true)

    fun boolOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Boolean? = null,
        required: Boolean = false,
    ) = declare(BoolOption(key, title, description, required, default))

    fun intOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Long? = null,
        required: Boolean = false,
        presets: List<OptionPreset<Long>> = emptyList(),
        allowCustom: Boolean = true,
    ): IntOption =
        declare(
            IntOption(
                key,
                title,
                description,
                required,
                default,
                presets.checked(key, allowCustom),
                allowCustom,
            )
        )

    @Deprecated(
        "Bundles built against 1.0 call intOption without presets",
        level = DeprecationLevel.HIDDEN,
    )
    fun intOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Long? = null,
        required: Boolean = false,
    ) = intOption(key, title, description, default, required, emptyList(), true)

    fun floatOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Double? = null,
        required: Boolean = false,
        presets: List<OptionPreset<Double>> = emptyList(),
        allowCustom: Boolean = true,
    ): FloatOption =
        declare(
            FloatOption(
                key,
                title,
                description,
                required,
                default,
                presets.checked(key, allowCustom),
                allowCustom,
            )
        )

    @Deprecated(
        "Bundles built against 1.0 call floatOption without presets",
        level = DeprecationLevel.HIDDEN,
    )
    fun floatOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Double? = null,
        required: Boolean = false,
    ) = floatOption(key, title, description, default, required, emptyList(), true)

    fun stringListOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: List<String>? = null,
        required: Boolean = false,
        presets: List<OptionPreset<List<String>>> = emptyList(),
        allowCustom: Boolean = true,
    ): StringListOption =
        declare(
            StringListOption(
                key,
                title,
                description,
                required,
                default,
                presets.checked(key, allowCustom),
                allowCustom,
            )
        )

    @Deprecated(
        "Bundles built against 1.0 call stringListOption without presets",
        level = DeprecationLevel.HIDDEN,
    )
    fun stringListOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: List<String>? = null,
        required: Boolean = false,
    ) = stringListOption(key, title, description, default, required, emptyList(), true)

    fun pathOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        required: Boolean = false,
    ) = declare(PathOption(key, title, description, required))

    private fun <O : Option<*>> declare(option: O): O = option.also { options += it }

    /** Registers settings with `host`, which becomes a dependency. */
    fun settings(host: SettingsHost, vararg contributions: SettingsContribution) {
        val declared = contributions.toList()
        settings(host) { declared }
    }

    @Deprecated(
        "Bundles built against 1.0 call settings with sections only",
        level = DeprecationLevel.HIDDEN,
    )
    fun settings(host: SettingsHost, vararg sections: SettingsSection) = settings(host, *sections)

    /**
     * Registers settings computed after `execute`, so they can depend on options. The block may
     * also return [stored] settings.
     */
    fun settings(host: SettingsHost, sections: PatchRuntime.() -> List<SettingsContribution>) {
        settings += host to sections
        if (host !in dependencies) dependencies += host
    }

    fun execute(block: PatchRuntime.() -> Unit) {
        executeBlock = block
    }

    fun afterDependents(block: PatchRuntime.() -> Unit) {
        afterDependentsBlock = block
    }

    /**
     * Works on the APK files the engine wrote, before it signs them. The engine has released the
     * patched app by then: [OutputRuntime] reaches only the files, the bundle and the options.
     */
    fun afterWrite(block: OutputRuntime.() -> Unit) {
        afterWriteBlock = block
    }

    internal fun build(): ReseamPatch =
        BuiltPatch(
            name,
            hidden,
            description,
            dependencies.toList(),
            compatibility.toList(),
            enabledByDefault,
            options.toList(),
            settings.toList(),
            executeBlock,
            afterDependentsBlock,
            afterWriteBlock,
        )
}

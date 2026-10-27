// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import app.reseam.patch.CompatiblePackage
import app.reseam.patch.PatchDeclaration
import app.reseam.patch.PatchRuntime
import app.reseam.patch.ReseamPatch
import app.reseam.patch.appEntry
import app.reseam.patch.run.ActiveRuntime

/**
 * The internal patch that gives one app the running bundle's settings screen. Patches register
 * their sections with it; after they have all run it writes the bundle's settings into the app,
 * links the app runtime, and runs the app-specific install.
 */
class SettingsHost
internal constructor(
    val appId: String,
    override val compatibleWith: List<CompatiblePackage>,
    override val dependencies: List<ReseamPatch>,
    private val ownSections: List<SettingsSection>,
    private val strings: List<StringTable>,
    private val brand: Brand,
    private val about: About?,
    private val install: PatchRuntime.() -> Unit,
) : ReseamPatch {
    override val name: String? = null
    override val description = "Settings for $appId"

    fun register(patch: ReseamPatch, sections: List<SettingsSection>) {
        if (sections.isEmpty()) return
        ActiveRuntime.current.afterSuccess {
            ActiveRuntime.current.run.register(this, patch, sections)
        }
    }

    /** The host runs before dependents, so its own sections lead the screen. */
    override fun execute(ctx: PatchRuntime) {
        ctx.run.clear(this)
        register(this, ownSections)
    }

    override fun afterDependents(ctx: PatchRuntime) {
        try {
            ctx.run.claimSettings(ctx.bundle.name, this)
            SettingsFiles(ctx.bundle, brand, about, ctx.run.sections(this), strings)
                .write(ctx.files)
            ctx.install()
            // appEntry code runs newest first, so this init precedes every other entry hook.
            appEntry { call(Reseam.init, application) }
        } finally {
            ctx.run.clear(this)
        }
    }

    override fun toString() = "$appId settings"
}

class SettingsHostBuilder internal constructor() : PatchDeclaration() {
    private var installBlock: PatchRuntime.() -> Unit = {}
    private val sections = mutableListOf<SettingsSection>()
    private val strings = mutableListOf<StringTable>()
    private var brand = Brand.DEFAULT
    private var about: About? = null

    /** Sections the host owns, for settings that belong to no single feature patch. */
    fun settings(vararg sections: SettingsSection) {
        this.sections += sections
    }

    /** String tables the bundle's extensions read; those the settings use ship regardless. */
    fun strings(vararg tables: StringTable) {
        strings += tables
    }

    /** How the settings screen and the components the bundle's extensions build look. */
    fun brand(brand: Brand) {
        this.brand = brand
    }

    /** Links on the settings screen's about card. */
    fun about(about: About) {
        this.about = about
    }

    /**
     * Wires the settings screen into the app, typically a row in the app's own settings that calls
     * [openSettings]; runs once every patch has registered its sections.
     */
    fun install(block: PatchRuntime.() -> Unit) {
        installBlock = block
    }

    internal fun build(appId: String) =
        SettingsHost(
            appId,
            compatibility.toList(),
            dependencies.toList(),
            sections.toList(),
            strings.toList(),
            brand,
            about,
            installBlock,
        )
}

fun settingsHost(appId: String, block: SettingsHostBuilder.() -> Unit): SettingsHost =
    SettingsHostBuilder().apply(block).build(appId)

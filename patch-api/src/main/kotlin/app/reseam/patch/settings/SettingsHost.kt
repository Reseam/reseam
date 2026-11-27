// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
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
 * their sections with it; after they have all run it writes the bundle's settings into the app and
 * runs the app-specific install.
 */
class SettingsHost
internal constructor(
    val appId: String,
    override val compatibleWith: List<CompatiblePackage>,
    override val dependencies: List<ReseamPatch>,
    private val ownSections: List<SettingsSection>,
    private val layout: List<SettingsContribution>,
    private val placements: Map<String, PagePlacement>,
    private val strings: List<StringTable>,
    private val brand: Brand,
    private val about: About?,
    private val install: PatchRuntime.() -> Unit,
) : ReseamPatch {
    override val name: StringRef? = null

    fun register(patch: ReseamPatch, sections: List<SettingsContribution>) {
        if (sections.isEmpty()) return
        ActiveRuntime.current.afterSuccess {
            ActiveRuntime.current.run.register(this, patch, sections)
        }
    }

    /**
     * Shows only the icons of the pages and settings with these keys on this run's settings screen.
     * The last patch to call it decides, and without a call every icon shows. Call it in `execute`.
     */
    fun showIconsOf(keys: Set<String>) = ActiveRuntime.current.run.showIcons(this, keys)

    /** The host runs before dependents, so its own sections lead the screen. */
    override fun execute(ctx: PatchRuntime) {
        appEntry { call(Reseam.init, application) }
        ctx.run.clear(this)
        register(this, ownSections)
    }

    override fun afterDependents(ctx: PatchRuntime) {
        try {
            ctx.run.claimSettings(ctx.bundle.name, this)
            val sections = ctx.run.sections(this)
            requireActionCode(sections)
            SettingsFiles(
                    ctx.bundle,
                    brand,
                    about,
                    sections,
                    layout,
                    placements,
                    strings,
                    ctx.run.iconKeys(this),
                )
                .write(ctx.files)
            ctx.install()
        } finally {
            ctx.run.clear(this)
        }
    }

    override fun toString() = "$appId settings"
}

class SettingsHostBuilder internal constructor() : PatchDeclaration() {
    private var installBlock: PatchRuntime.() -> Unit = {}
    private val sections = mutableListOf<SettingsSection>()
    private val layout = mutableListOf<SettingsContribution>()
    private val placements = mutableMapOf<String, PagePlacement>()
    private val strings = mutableListOf<StringTable>()
    private var brand = Brand.DEFAULT
    private var about: About? = null

    /** Sections the host owns, for settings that belong to no single feature patch. */
    fun settings(vararg sections: SettingsSection) {
        this.sections += sections
    }

    /**
     * Where and in what order the settings patches show appear. A shown setting listed here takes
     * its section's page, heading and place, and these sections come first on their page, in this
     * order. One listed as [stored] loses its row and keeps its value. Listing a setting does not
     * show it: one no patch shows stays off the screen. Shown settings left out stay in their
     * patch's section, after the listed ones. A setting may appear on several pages, once on each.
     */
    fun layout(vararg contributions: SettingsContribution) {
        layout += contributions
    }

    /**
     * Shows [page]'s row on its parent page right after [after]'s row, while that setting shows,
     * instead of with the page rows that lead the parent. With a [heading], the row starts a new
     * group under it.
     */
    fun place(page: SettingsPage, after: Setting<*>, heading: StringRef? = null) {
        placements[page.id] = PagePlacement(after, heading)
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
            layout.toList(),
            placements.toMap(),
            strings.toList(),
            brand,
            about,
            installBlock,
        )
}

fun settingsHost(appId: String, block: SettingsHostBuilder.() -> Unit): SettingsHost =
    SettingsHostBuilder().apply(block).build(appId)

internal class PagePlacement(val after: Setting<*>, val heading: StringRef?)

// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.ReseamPatch
import app.reseam.patch.appEntry
import app.reseam.patch.patch
import app.reseam.patch.settings.About
import app.reseam.patch.settings.Link
import app.reseam.patch.settings.SettingsGroup
import app.reseam.patch.settings.SettingsPage
import app.reseam.patch.settings.Slider
import app.reseam.patch.settings.TakesEffect
import app.reseam.patch.settings.Vector
import app.reseam.patch.settings.Verbatim
import app.reseam.patch.settings.action
import app.reseam.patch.settings.color
import app.reseam.patch.settings.enabledWhile
import app.reseam.patch.settings.note
import app.reseam.patch.settings.noteOf
import app.reseam.patch.settings.number
import app.reseam.patch.settings.onAction
import app.reseam.patch.settings.onChange
import app.reseam.patch.settings.onEnabled
import app.reseam.patch.settings.onSummary
import app.reseam.patch.settings.onValidate
import app.reseam.patch.settings.range
import app.reseam.patch.settings.section
import app.reseam.patch.settings.settingsHost
import app.reseam.patch.settings.stored
import app.reseam.patch.settings.text
import app.reseam.patch.settings.toggle
import app.reseam.patch.settings.whenEnabled

private val chevron =
    Vector.svg(
        """<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"
            stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>"""
    )

private val dot =
    Vector.svg(
        """<svg xmlns="http://www.w3.org/2000/svg" viewBox="10 10 4 4">
            <circle cx="12" cy="12" r="2" fill="#f00"/></svg>"""
    )

private val navigationRoot =
    SettingsPage("media", TestStrings.pageMedia, order = 10, icon = chevron)
private val navigationChild =
    SettingsPage("quality", TestStrings.pageQuality, navigationRoot, order = 11)
private val navigationOther = SettingsPage("general", TestStrings.pageGeneral, order = 0)
private val navigationEmpty = SettingsPage("empty", TestStrings.pageEmpty)

object NavigationSettings : SettingsGroup(TestStrings) {
    val rootEnabled by toggle(default = false, key = "root.enabled", icon = dot)
    val rootLines by
        text(
            default = "",
            multiline = true,
            key = "root.lines",
            takesEffect = TakesEffect.IMMEDIATELY,
        )
    val qualityFirst by toggle(default = true, key = "quality.first")
    val qualitySecond by toggle(default = false, key = "quality.second")
    val generalEnabled by toggle(default = true, key = "general.enabled")
    val gateEnabled by toggle(default = true, key = "gate.enabled")
    val share by action(key = "share", icon = dot)
}

val navigationSettings =
    settingsHost("navigation") {
        compatibleWith("com.example.test")
        about(
            About(
                links =
                    listOf(
                        Link(Verbatim("Forum"), "https://example.com/forum", chevron),
                        Link(
                            Verbatim("Chat"),
                            "https://example.com/chat",
                            Vector.lucide("chevron-right"),
                        ),
                    ),
                actions = listOf(Link(TestStrings.pageMedia, "https://example.com/donate", dot)),
            )
        )
        settings(
            section(
                TestStrings.sectionRoot,
                NavigationSettings.rootEnabled,
                NavigationSettings.rootLines,
            )
        )
    }

val navigationFirstPatch =
    patch(TestStrings.settingsNavigationFirst) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(
            navigationSettings,
            section(navigationChild, TestStrings.sectionQuality, NavigationSettings.qualityFirst),
            section(navigationEmpty, TestStrings.sectionEmpty),
        )
    }

val navigationSecondPatch =
    patch(TestStrings.settingsNavigationSecond) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        dependsOn(navigationFirstPatch)
        settings(
            navigationSettings,
            section(
                navigationChild,
                TestStrings.sectionQuality,
                NavigationSettings.qualityFirst.copy(default = false),
                NavigationSettings.qualitySecond,
            ),
            section(navigationOther, TestStrings.sectionGeneral, NavigationSettings.generalEnabled),
        )
    }

val settingsIconKeys =
    patch(TestStrings.settingsIconKeys) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        dependsOn(navigationFirstPatch)
        execute { navigationSettings.showIconsOf(setOf("media")) }
    }

val settingsRowBehaviour =
    patch(TestStrings.settingsRowBehaviour) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(
            navigationSettings,
            section(
                TestStrings.sectionGate,
                NavigationSettings.rootEnabled,
                NavigationSettings.rootLines,
            ),
        )
        execute {
            enabledWhile(NavigationSettings.rootLines, NavigationSettings.rootEnabled)
            onSummary(NavigationSettings.rootLines) { returnValue(value) }
            onValidate(NavigationSettings.rootLines) { returnNull() }
            onChange(NavigationSettings.rootEnabled) {}
            onEnabled(NavigationSettings.rootEnabled) { returnTrue() }
        }
    }

val gatedEntry =
    patch(TestStrings.settingsGatedEntry) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(
            navigationSettings,
            section(TestStrings.sectionGate, NavigationSettings.gateEnabled),
        )
        execute {
            appEntry {
                whenEnabled(NavigationSettings.gateEnabled) {
                    callStatic(
                        "com.example.Observer",
                        "started",
                        "(Landroid/app/Application;)V",
                        application,
                    )
                }
            }
        }
    }

val actionRow =
    patch(TestStrings.settingsAction) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(navigationSettings, section(TestStrings.sectionActions, NavigationSettings.share))
        execute {
            onAction(NavigationSettings.share) {
                callStatic("com.example.Observer", "shared", "(Landroid/app/Activity;)V", activity)
            }
        }
    }

val uncodedActionRow =
    patch(TestStrings.settingsActionWithoutCode) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(navigationSettings, section(TestStrings.sectionActions, NavigationSettings.share))
    }

private val layoutPage = SettingsPage("layout", TestStrings.pageLayout)

object LayoutSettings : SettingsGroup(TestStrings) {
    val layoutFirst by toggle(default = false, key = "layout.first")
    val layoutSecond by toggle(default = false, key = "layout.second")
    val layoutHidden by toggle(default = false, key = "layout.hidden")
    val layoutStored by text(default = "", key = "layout.stored")
    val layoutLevel by
        number(default = 5.0, slider = Slider(0.0, 10.0, 1.0, "x"), key = "layout.level")
    val layoutTint by color(default = 0xFFFF0000, key = "layout.tint")
    val layoutNote by note(key = "layout.note")
    val layoutRange by
        range(default = 0.2..0.8, slider = Slider(0.0, 1.0, 0.1), key = "layout.range")
}

private val layoutRunPage = SettingsPage("layout.run", TestStrings.pageLayout, parent = layoutPage)

val layoutSettings =
    settingsHost("layout") {
        compatibleWith("com.example.test")
        layout(
            section(layoutPage, LayoutSettings.layoutSecond, LayoutSettings.layoutNote),
            section(layoutPage, TestStrings.sectionGeneral, LayoutSettings.layoutFirst),
            section(navigationOther, LayoutSettings.layoutFirst),
            stored(LayoutSettings.layoutHidden),
        )
        place(
            layoutRunPage,
            after = LayoutSettings.layoutFirst,
            heading = TestStrings.sectionGeneral,
        )
    }

val layoutRunNotes: ReseamPatch = patch {
    dependsOn(layoutSettings)
    afterDependents {
        layoutSettings.register(
            layoutRunNotes,
            listOf(section(layoutRunPage, noteOf("layout.run", TestStrings.pageLayout, null))),
        )
    }
}

val settingsLayout =
    patch(TestStrings.settingsLayout) {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        dependsOn(layoutRunNotes)
        settings(
            layoutSettings,
            section(
                layoutPage,
                TestStrings.sectionQuality,
                LayoutSettings.layoutFirst,
                LayoutSettings.layoutSecond,
                LayoutSettings.layoutHidden,
                LayoutSettings.layoutLevel,
                LayoutSettings.layoutTint,
                LayoutSettings.layoutNote,
                LayoutSettings.layoutRange,
            ),
            stored(LayoutSettings.layoutStored),
        )
    }

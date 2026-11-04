// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.appEntry
import app.reseam.patch.patch
import app.reseam.patch.settings.About
import app.reseam.patch.settings.Link
import app.reseam.patch.settings.SettingsGroup
import app.reseam.patch.settings.SettingsPage
import app.reseam.patch.settings.Vector
import app.reseam.patch.settings.Verbatim
import app.reseam.patch.settings.action
import app.reseam.patch.settings.onAction
import app.reseam.patch.settings.section
import app.reseam.patch.settings.settingsHost
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
    val rootLines by text(default = "", multiline = true, key = "root.lines")
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
                links = listOf(Link(Verbatim("Forum"), "https://example.com/forum", chevron)),
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
    patch("Settings navigation first") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(
            navigationSettings,
            section(navigationChild, TestStrings.sectionQuality, NavigationSettings.qualityFirst),
            section(navigationEmpty, TestStrings.sectionEmpty),
        )
    }

val navigationSecondPatch =
    patch("Settings navigation second") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        dependsOn(navigationFirstPatch)
        settings(
            navigationSettings,
            section(
                navigationChild,
                TestStrings.sectionQuality,
                NavigationSettings.qualityFirst,
                NavigationSettings.qualitySecond,
            ),
            section(navigationOther, TestStrings.sectionGeneral, NavigationSettings.generalEnabled),
        )
    }

val gatedEntry =
    patch("Settings gated entry") {
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
    patch("Settings action") {
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
    patch("Settings action without code") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(navigationSettings, section(TestStrings.sectionActions, NavigationSettings.share))
    }

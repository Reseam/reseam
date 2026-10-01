// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.patch
import app.reseam.patch.settings.SettingsPage
import app.reseam.patch.settings.TextSetting
import app.reseam.patch.settings.ToggleSetting
import app.reseam.patch.settings.section
import app.reseam.patch.settings.settingsHost

private val navigationRoot = SettingsPage("media", "Media", order = 10)
private val navigationChild = SettingsPage("quality", "Quality", navigationRoot, order = 11)
private val navigationOther = SettingsPage("general", "General", order = 0)
private val navigationEmpty = SettingsPage("empty", "Empty")
private val navigationFirst = ToggleSetting("quality.first", "First", default = true)
private val navigationSecond = ToggleSetting("quality.second", "Second", default = false)

val navigationSettings =
    settingsHost("navigation") {
        compatibleWith("com.example.test")
        settings(
            section(
                "Root",
                ToggleSetting("root.enabled", "Root", default = false),
                TextSetting("root.lines", "Lines", default = "", multiline = true),
            )
        )
    }

val navigationFirstPatch =
    patch("Settings navigation first") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        settings(
            navigationSettings,
            section(navigationChild, "Quality", navigationFirst),
            section(navigationEmpty, "Empty"),
        )
    }

val navigationSecondPatch =
    patch("Settings navigation second") {
        compatibleWith("com.example.test")
        enabledByDefault(false)
        dependsOn(navigationFirstPatch)
        settings(
            navigationSettings,
            section(navigationChild, "Quality", navigationFirst, navigationSecond),
            section(
                navigationOther,
                "General",
                ToggleSetting("general.enabled", "General", default = true),
            ),
        )
    }

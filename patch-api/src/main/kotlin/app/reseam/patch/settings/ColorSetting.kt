// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/**
 * A color picked in a color picker, as ARGB. Without [opacity] it is always opaque. A [choice] the
 * color belongs with, such as what a colored segment does, is picked above the color in the same
 * dialog and saved with it; it takes no row of its own.
 */
class ColorSetting
internal constructor(
    key: String,
    internal override val property: String,
    override val title: StringRef,
    summary: StringRef?,
    default: Int,
    icon: Vector?,
    takesEffect: TakesEffect,
    val opacity: Boolean,
    val choice: ChoiceSetting?,
) : Setting<Int>(SettingKind.COLOR, key, property, title, summary, default, icon, takesEffect) {
    init {
        require(opacity || default ushr 24 == OPAQUE) {
            "Color setting '$key' has no opacity, so its default needs alpha FF"
        }
    }
}

private const val OPAQUE = 0xFF

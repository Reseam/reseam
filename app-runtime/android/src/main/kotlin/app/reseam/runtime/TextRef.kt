// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

/** Text from a bundle's settings schema, resolved in the app's current language when shown. */
internal sealed interface DisplayText {
    fun resolve(bundle: String): String
}

internal data class TextRef(val table: String, val name: String) : DisplayText {
    override fun resolve(bundle: String): String =
        StringTable("reseam/$bundle/strings/$table")[name]
}

internal data class Verbatim(val text: String) : DisplayText {
    override fun resolve(bundle: String): String = text
}

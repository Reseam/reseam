// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import java.util.concurrent.ConcurrentHashMap

internal sealed interface DisplayText {
    fun resolve(bundle: String): CharSequence
}

internal data class TextRef(val table: String, val name: String) : DisplayText {
    override fun resolve(bundle: String): CharSequence =
        tables.computeIfAbsent("reseam/$bundle/strings/$table", ::StringTable).text(name)
}

internal data class Verbatim(val text: String) : DisplayText {
    override fun resolve(bundle: String): CharSequence = text
}

private val tables = ConcurrentHashMap<String, StringTable>()

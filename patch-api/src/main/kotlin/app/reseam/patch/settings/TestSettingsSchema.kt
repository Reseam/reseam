// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import java.io.File

/**
 * Writes the settings schema the app runtime reads in JVM tests of a bundle's extensions: every
 * setting of the compiled settings groups, stored without a row, as patching stores them.
 * Arguments: output file, bundle name, class directories.
 */
internal fun main(args: Array<String>) {
    val settings =
        settingsGroups(args.drop(2).map(::File)).flatMap(::valueSettings).distinctBy { it.key }
    val schema =
        settingsSchema(
            SchemaSource(args[1], null, "test", null),
            Brand.DEFAULT,
            null,
            null,
            SchemaScreen(emptyList(), emptyList(), settings, emptyMap(), RowWiring.NONE) { _, icon
                ->
                icon
            },
        )
    File(args[0]).apply { parentFile.mkdirs() }.writeText(schema)
}

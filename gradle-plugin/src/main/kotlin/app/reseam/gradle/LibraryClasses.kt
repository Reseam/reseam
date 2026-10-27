// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.io.InputStream
import java.util.zip.ZipFile

/** A compiled class in a library: its path inside the library and where it came from. */
internal class LibraryClass(
    val path: String,
    val location: String,
    private val open: () -> InputStream,
) {
    fun <T> read(block: (InputStream) -> T): T = open().use(block)
}

/** Visits every class of [library], a class folder or a jar, while the library is open. */
internal fun <T> libraryClasses(library: File, visit: (Sequence<LibraryClass>) -> T): T =
    if (library.isDirectory) {
        visit(
            library.classFiles().map { file ->
                LibraryClass(file.relativeTo(library).invariantSeparatorsPath, file.path) {
                    file.inputStream()
                }
            }
        )
    } else {
        ZipFile(library).use { jar ->
            visit(
                jar.entries()
                    .asSequence()
                    .filter { !it.isDirectory && it.name.endsWith(".class") }
                    .map { entry ->
                        LibraryClass(entry.name, "${entry.name} in $library") {
                            jar.getInputStream(entry)
                        }
                    }
            )
        }
    }

/** The class paths of [libraries], as a jar task excludes them. */
internal fun classPaths(libraries: Iterable<File>): List<String> = libraries.flatMap { library ->
    libraryClasses(library) { classes -> classes.map { it.path }.toList() }
}

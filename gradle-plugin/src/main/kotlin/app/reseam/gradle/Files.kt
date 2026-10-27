// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.gradle.api.GradleException
import org.gradle.api.file.FileCollection

/** Empties [dir], creating it when missing. */
internal fun recreate(dir: File) {
    if (dir.exists() && !dir.deleteRecursively()) throw GradleException("cannot remove $dir")
    if (!dir.mkdirs() && !dir.isDirectory) throw GradleException("cannot create $dir")
}

internal fun File.classFiles(): Sequence<File> =
    walkTopDown()
        .onFail { file, error -> throw GradleException("cannot read classes in $file", error) }
        .filter { it.isFile && it.extension == "class" }

internal fun FileCollection.classFiles(): List<File> =
    files.filter(File::isDirectory).flatMap { it.classFiles() }

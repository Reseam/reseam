// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import org.gradle.api.GradleException
import org.gradle.api.initialization.Settings

/** An app's or a shared folder's modules, by project path. */
internal class Folder(
    val name: String,
    val dir: File,
    val patch: String?,
    val settings: String?,
    val extensions: List<Extension>,
) {
    fun paths() = listOfNotNull(patch, settings) + extensions.map { it.path }

    /** Whether the folder's extensions are TypeScript checked; see [registerTypecheck]. */
    val typed: Boolean
        get() = File(dir, "package.json").isFile

    fun artifact(path: String): String =
        when (path) {
            patch -> name
            settings -> "$name-settings"
            else -> extensions.single { it.path == path }.artifact
        }

    fun artifacts() =
        listOfNotNull(
            patch?.let { "$name-patches.jar" },
            settings?.let { "$name-settings.dex" },
        ) + extensions.map { "${it.artifact}.dex" }
}

internal class Extension(val path: String, val artifact: String, val javascript: Boolean)

internal fun Settings.appFolder(app: File): Folder {
    val path = ":apps:${app.name}"
    return Folder(
        app.name,
        app,
        File(app, "patch").takeIf { it.isDirectory }?.let { include("$path:patch", it) },
        File(app, "settings").takeIf { it.isDirectory }?.let { include("$path:settings", it) },
        directories(File(app, "extensions")).map {
            Extension(
                include("$path:extensions:${it.name}", it),
                "${app.name}-${it.name}",
                isJavascript(it),
            )
        },
    )
}

/**
 * A shared folder is an extension, patches for several apps in `patch/`, or both, with optional
 * settings.
 */
internal fun Settings.sharedFolder(shared: File): Folder {
    val path = ":shared:${shared.name}"
    val patch = File(shared, "patch").takeIf { it.isDirectory }
    val extension =
        if (patch == null || File(shared, "src").isDirectory)
            Extension(include(path, shared), shared.name, isJavascript(shared))
        else null
    return Folder(
        shared.name,
        shared,
        patch?.let { include("$path:patch", it) },
        File(shared, "settings").takeIf { it.isDirectory }?.let { include("$path:settings", it) },
        listOfNotNull(extension),
    )
}

private fun isJavascript(module: File) = File(module, "src/main/js").isDirectory

private fun Settings.include(path: String, dir: File): String {
    include(path)
    project(path).projectDir = dir
    return path
}

internal fun directories(parent: File): List<File> =
    (if (!parent.exists()) emptyArray()
        else parent.listFiles() ?: throw GradleException("cannot read workspace directory $parent"))
        .filter { it.isDirectory && !it.name.startsWith(".") && it.name != "build" }
        .sortedBy { it.name }

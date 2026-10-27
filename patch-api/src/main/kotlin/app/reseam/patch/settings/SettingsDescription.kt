// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import java.io.File

/**
 * Describes compiled settings groups for the Gradle plugin. Arguments: output file, class
 * directories.
 */
internal fun main(args: Array<String>) {
    val groups =
        args
            .drop(1)
            .flatMap { classNames(File(it)) }
            .map { Class.forName(it, false, SettingsGroup::class.java.classLoader) }
            .filter {
                SettingsGroup::class.java.isAssignableFrom(it) && it != SettingsGroup::class.java
            }
            .sortedBy { it.name }
    File(args[0]).writeText(Json.build { obj { field("groups") { array(groups) { group(it) } } } })
}

private fun classNames(root: File): List<String> =
    root
        .walkTopDown()
        .filter { it.isFile && it.extension == "class" && !it.name.contains('$') }
        .map {
            it.relativeTo(root).invariantSeparatorsPath.removeSuffix(".class").replace('/', '.')
        }
        .toList()

private fun Json.group(type: Class<*>) {
    val instance = type.getField("INSTANCE").get(null)
    val settings =
        type.methods
            .filter {
                it.parameterCount == 0 && Setting::class.java.isAssignableFrom(it.returnType)
            }
            .map { it.invoke(instance) as Setting<*> }
            .sortedBy { it.property }
    obj {
        field("class", type.name)
        field("settings") { array(settings) { setting(it) } }
    }
}

private fun Json.setting(setting: Setting<*>) = obj {
    field("property", setting.property)
    field("key", setting.key)
    field("valueClass", setting.kind.runtimeValue)
    when (val default = setting.default) {
        is Boolean -> field("default", default)
        is String -> field("default", default)
    }
}

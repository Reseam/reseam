// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import java.io.File

/**
 * Describes compiled settings groups for the Gradle plugin. Arguments: output file, class
 * directories.
 */
internal fun main(args: Array<String>) {
    val groups = settingsGroups(args.drop(1).map(::File))
    File(args[0]).writeText(Json.build { obj { field("groups") { array(groups) { group(it) } } } })
}

internal fun settingsGroups(roots: List<File>): List<Class<*>> =
    roots
        .flatMap(::classNames)
        .map { Class.forName(it, false, SettingsGroup::class.java.classLoader) }
        .filter {
            SettingsGroup::class.java.isAssignableFrom(it) && it != SettingsGroup::class.java
        }
        .sortedBy { it.name }

internal fun valueSettings(group: Class<*>): List<Setting<*>> {
    val instance = group.getField("INSTANCE").get(null)
    return group.methods
        .filter { it.parameterCount == 0 && Setting::class.java.isAssignableFrom(it.returnType) }
        .map { it.invoke(instance) as Setting<*> }
        .filter { it.kind.runtimeValue != null }
        .sortedBy { it.property }
}

private fun classNames(root: File): List<String> =
    root
        .walkTopDown()
        .filter { it.isFile && it.extension == "class" && !it.name.contains('$') }
        .map {
            it.relativeTo(root).invariantSeparatorsPath.removeSuffix(".class").replace('/', '.')
        }
        .toList()

private fun Json.group(type: Class<*>) = obj {
    field("class", type.name)
    field("settings") { array(valueSettings(type)) { setting(it) } }
}

private fun Json.setting(setting: Setting<*>) = obj {
    field("property", setting.property)
    field("key", setting.key)
    field("valueClass", setting.kind.runtimeValue)
    when (setting) {
        is ToggleSetting -> field("default", setting.default)
        is NumberSetting -> field("default", setting.default)
        is RangeSetting -> field("default") { range(setting.default) }
        is ColorSetting -> field("default", setting.default.toUInt().toLong())
        is TextSetting -> field("default", setting.default)
        is FolderSetting -> field("default", setting.default)
        is ChoiceSetting -> field("default", setting.default)
        is FileSetting -> {
            field("default", setting.default)
            field("mimeTypes") { array(setting.mimeTypes) { value(it) } }
        }
        is ActionSetting,
        is NoteSetting -> error("$setting holds no value")
    }
}

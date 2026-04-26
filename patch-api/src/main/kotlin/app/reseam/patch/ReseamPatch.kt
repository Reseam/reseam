// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.settings.SettingsSection

class PatchException(message: String) : Exception(message)

enum class PatchOptionType {
    STRING,
    BOOL,
    INT,
    FLOAT,
    STRING_LIST,
    PATH,
}

data class PatchOption(
    val key: String,
    val title: String = key,
    val description: String = "",
    val type: PatchOptionType,
    val defaultString: String? = null,
    val defaultBool: Boolean? = null,
    val defaultInt: Long? = null,
    val defaultFloat: Double? = null,
    val defaultStringList: List<String>? = null,
    val validValues: List<String>? = null,
    val required: Boolean = false,
)

interface ReseamPatch {
    val name: String
    val description: String get() = ""
    val dependencies: List<String> get() = emptyList()
    val compatibleWith: List<CompatiblePackage> get() = emptyList()
    // This controls default selection only. Callers can still explicitly enable or disable patches.
    val enabled: Boolean get() = true
    // Declarations are metadata; execution-time values are supplied by the host runtime.
    val options: List<PatchOption> get() = emptyList()
    val extensionDex: List<String> get() = emptyList()
    val settingsHost: String? get() = null
    val settings: List<SettingsSection> get() = emptyList()

    fun execute(ctx: PatchRuntime)
    fun afterDependents(ctx: PatchRuntime) {}
}

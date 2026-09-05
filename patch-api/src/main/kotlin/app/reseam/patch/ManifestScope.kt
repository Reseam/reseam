// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

class ManifestScope internal constructor(
    private val componentName: String? = null,
) {
    fun components(): List<String> = componentNames()

    fun component(name: String): ManifestScope = ManifestScope(name)

    val packageName: String? get() = manifestPackageName(componentName)
    val versionCode: UInt? get() = manifestVersionCode(componentName)
    val versionName: String? get() = manifestVersionName(componentName)
    val minSdkVersion: UInt? get() = manifestMinSdkVersion(componentName)
    val splitName: String? get() = manifestSplitName(componentName)

    fun setVersionCode(code: UInt) = manifestSetVersionCode(componentName, code)

    fun setVersionName(name: String) = manifestSetVersionName(componentName, name)

    fun setMinSdk(sdk: UInt) = manifestSetMinSdk(componentName, sdk)

    fun addPermission(permission: String) = manifestAddPermission(componentName, permission)

    fun setAttributeInt(elementName: String, attrName: String, value: Int) =
        manifestSetAttributeInt(componentName, elementName, attrName, value)

    fun setAttributeString(elementName: String, attrName: String, value: String) =
        manifestSetAttributeString(componentName, elementName, attrName, value)

    fun setActivityConfigChanges(activityName: String, configChanges: String) =
        manifestSetActivityConfigChanges(componentName, activityName, configChanges)

    fun addIntentFilter(
        activityName: String,
        action: String? = null,
        category: String? = null,
        mimeType: String? = null,
    ) = manifestAddIntentFilter(componentName, activityName, action, category, mimeType)

    fun addActivityAlias(
        targetActivity: String,
        aliasName: String,
        enabled: Boolean = true,
        label: String? = null,
    ) = manifestAddActivityAlias(componentName, targetActivity, aliasName, enabled, label)

    fun copyIntentFilters(fromActivity: String, toActivity: String) =
        manifestCopyIntentFilters(componentName, fromActivity, toActivity)

    fun document(): XmlDocument {
        val handle = manifestGetDocument(componentName) ?: error("manifest not available")
        return XmlDocument(handle)
    }
}

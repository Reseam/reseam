// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

class ManifestScope internal constructor(
    private val componentName: String? = null,
) {
    fun components(): List<String> = manifestComponentNames()

    fun component(name: String): ManifestScope = ManifestScope(name)

    val packageName: String?
        get() = componentName?.let(::manifestPackageNameInComponent) ?: manifestPackageName()

    val versionCode: UInt?
        get() = componentName?.let(::manifestVersionCodeInComponent) ?: manifestVersionCode()

    val versionName: String?
        get() = componentName?.let(::manifestVersionNameInComponent) ?: manifestVersionName()

    val minSdkVersion: UInt?
        get() = componentName?.let(::manifestMinSdkVersionInComponent) ?: manifestMinSdkVersion()

    val splitName: String?
        get() = componentName?.let(::manifestSplitNameInComponent) ?: manifestSplitName()

    fun setVersionCode(code: UInt) {
        if (componentName == null) {
            manifestSetVersionCode(code)
        } else {
            manifestSetVersionCodeInComponent(componentName, code)
        }
    }

    fun setVersionName(name: String) {
        if (componentName == null) {
            manifestSetVersionName(name)
        } else {
            manifestSetVersionNameInComponent(componentName, name)
        }
    }

    fun setMinSdk(sdk: UInt) {
        if (componentName == null) {
            manifestSetMinSdk(sdk)
        } else {
            manifestSetMinSdkInComponent(componentName, sdk)
        }
    }

    fun addPermission(permission: String) {
        if (componentName == null) {
            manifestAddPermission(permission)
        } else {
            manifestAddPermissionInComponent(componentName, permission)
        }
    }

    fun setAttributeInt(elementName: String, attrResId: Int, value: Int) {
        if (componentName == null) {
            manifestSetAttributeInt(elementName, attrResId.toUInt(), value)
        } else {
            manifestSetAttributeIntInComponent(componentName, elementName, attrResId.toUInt(), value)
        }
    }

    fun setAttributeString(elementName: String, attrResId: Int, value: String) {
        if (componentName == null) {
            manifestSetAttributeString(elementName, attrResId.toUInt(), value)
        } else {
            manifestSetAttributeStringInComponent(componentName, elementName, attrResId.toUInt(), value)
        }
    }

    fun setActivityConfigChanges(activityName: String, configChanges: String) {
        if (componentName == null) {
            manifestSetActivityConfigChanges(activityName, configChanges)
        } else {
            manifestSetActivityConfigChangesInComponent(componentName, activityName, configChanges)
        }
    }

    fun addIntentFilter(
        activityName: String,
        action: String? = null,
        category: String? = null,
        mimeType: String? = null,
    ) {
        if (componentName == null) {
            manifestAddIntentFilter(activityName, action, category, mimeType)
        } else {
            manifestAddIntentFilterInComponent(componentName, activityName, action, category, mimeType)
        }
    }

    fun addActivityAlias(
        targetActivity: String,
        aliasName: String,
        enabled: Boolean = true,
        label: String? = null,
    ) {
        if (componentName == null) {
            manifestAddActivityAlias(targetActivity, aliasName, enabled, label)
        } else {
            manifestAddActivityAliasInComponent(componentName, targetActivity, aliasName, enabled, label)
        }
    }

    fun copyIntentFilters(fromActivity: String, toActivity: String) {
        if (componentName == null) {
            manifestCopyIntentFilters(fromActivity, toActivity)
        } else {
            manifestCopyIntentFiltersInComponent(componentName, fromActivity, toActivity)
        }
    }

    fun document(): XmlDocument {
        val handle = componentName?.let(::manifestGetDocumentInComponent) ?: manifestGetDocument()
        return XmlDocument(handle)
    }
}

// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.native.componentNames
import app.reseam.patch.native.manifestAddActivityAlias
import app.reseam.patch.native.manifestAddIntentFilter
import app.reseam.patch.native.manifestAddPermission
import app.reseam.patch.native.manifestCopyIntentFilters
import app.reseam.patch.native.manifestGetDocument
import app.reseam.patch.native.manifestMinSdkVersion
import app.reseam.patch.native.manifestPackageName
import app.reseam.patch.native.manifestSetActivityConfigChanges
import app.reseam.patch.native.manifestSetAttributeInt
import app.reseam.patch.native.manifestSetAttributeString
import app.reseam.patch.native.manifestSetMinSdk
import app.reseam.patch.native.manifestSetVersionCode
import app.reseam.patch.native.manifestSetVersionName
import app.reseam.patch.native.manifestSplitName
import app.reseam.patch.native.manifestVersionCode
import app.reseam.patch.native.manifestVersionName

class ManifestScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = componentNames()

    fun component(name: String): ManifestScope = ManifestScope(name)

    val packageName: String?
        get() = manifestPackageName(componentName)

    val versionCode: UInt?
        get() = manifestVersionCode(componentName)

    val versionName: String?
        get() = manifestVersionName(componentName)

    val minSdkVersion: UInt?
        get() = manifestMinSdkVersion(componentName)

    val splitName: String?
        get() = manifestSplitName(componentName)

    /**
     * The fully qualified `<application android:name>`, or null when the app uses the platform
     * default.
     */
    val applicationClass: String?
        get() = edit {
            val name =
                findByTag("application").firstOrNull()?.get("android:name") ?: return@edit null
            val pkg = packageName ?: return@edit name
            when {
                name.startsWith(".") -> pkg + name
                '.' !in name -> "$pkg.$name"
                else -> name
            }
        }

    fun setVersionCode(code: UInt) = manifestSetVersionCode(componentName, code)

    fun setVersionName(name: String) = manifestSetVersionName(componentName, name)

    fun setMinSdk(sdk: UInt) = manifestSetMinSdk(componentName, sdk)

    fun addPermission(permission: String) = manifestAddPermission(componentName, permission)

    fun setAttributeInt(elementName: String, attrName: String, value: Int) =
        manifestSetAttributeInt(componentName, elementName, attrName, value)

    /**
     * Sets an `android:` attribute of the first [elementName]. `@type/name` and `?attr` become
     * references to the base resource table and the attribute's enum and flag names (`singleTask`,
     * `orientation|screenSize`) become its values, as aapt writes them; other text stays a string,
     * and a leading `\\@` or `\\?` keeps the character.
     */
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

    /**
     * Appends an `<activity-alias>` for [targetActivity] at the end of `<application>`, after the
     * activity it names as Android requires. [label] is read like [setAttributeString] values, so
     * `@string/name` is a reference.
     */
    fun addActivityAlias(
        targetActivity: String,
        aliasName: String,
        enabled: Boolean = true,
        label: String? = null,
    ) = manifestAddActivityAlias(componentName, targetActivity, aliasName, enabled, label)

    /**
     * Copies every intent filter of [fromActivity] to [toActivity]; either may be an
     * `<activity-alias>`.
     */
    fun copyIntentFilters(fromActivity: String, toActivity: String) =
        manifestCopyIntentFilters(componentName, fromActivity, toActivity)

    /**
     * Declares an activity, unexported, unless the manifest already has it. `block` sets further
     * attributes.
     */
    fun addActivity(name: String, block: XmlElement.() -> Unit = {}) = edit {
        val application =
            findByTag("application").firstOrNull() ?: error("manifest has no <application> element")
        if (findByAttribute("android:name", name).isNotEmpty()) return@edit
        application.appendChild(
            createElement("activity").apply {
                this["android:name"] = name
                this["android:exported"] = "false"
                block()
            }
        )
    }

    fun document(): XmlDocument =
        XmlDocument(manifestGetDocument(componentName) ?: error("manifest not available"))

    fun <T> edit(block: XmlDocument.() -> T): T = document().use(block)
}

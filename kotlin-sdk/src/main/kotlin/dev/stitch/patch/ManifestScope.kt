@file:Suppress("unused")

package dev.stitch.patch

object Manifest {
    val packageName: String get() = manifestPackageName()
    val versionCode: Int? get() = manifestVersionCode()?.toInt()
    val versionName: String? get() = manifestVersionName()
    val minSdkVersion: Int? get() = manifestMinSdkVersion()?.toInt()
    val splitName: String? get() = manifestSplitName()

    fun setVersionCode(code: Int) = manifestSetVersionCode(code.toUInt())
    fun setVersionName(name: String) = manifestSetVersionName(name)
    fun setMinSdk(sdk: Int) = manifestSetMinSdk(sdk.toUInt())
    fun addPermission(permission: String) = manifestAddPermission(permission)

    fun setAttributeInt(elementName: String, attrResId: Int, value: Int) =
        manifestSetAttributeInt(elementName, attrResId.toUInt(), value)

    fun setAttributeString(elementName: String, attrResId: Int, value: String) =
        manifestSetAttributeString(elementName, attrResId.toUInt(), value)

    fun setActivityConfigChanges(activityName: String, configChanges: String) =
        manifestSetActivityConfigChanges(activityName, configChanges)

    fun addIntentFilter(
        activityName: String,
        action: String? = null,
        category: String? = null,
        mimeType: String? = null,
    ) = manifestAddIntentFilter(activityName, action, category, mimeType)

    fun addActivityAlias(
        targetActivity: String,
        aliasName: String,
        enabled: Boolean = true,
        label: String? = null,
    ) = manifestAddActivityAlias(targetActivity, aliasName, enabled, label)

    fun copyIntentFilters(fromActivity: String, toActivity: String) =
        manifestCopyIntentFilters(fromActivity, toActivity)

    fun document(): XmlDocument = XmlDocument(manifestGetDocument())
}

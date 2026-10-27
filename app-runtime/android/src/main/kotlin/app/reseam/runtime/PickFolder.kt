// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.util.Log

/** Stores the picked folder as setting [key] of [bundle], with access that survives restarts. */
internal data class PickFolder(val bundle: String, val key: String) : DocumentAction {
    override fun complete(activity: Activity, uri: Uri) {
        try {
            activity.contentResolver.takePersistableUriPermission(uri, FOLDER_ACCESS)
        } catch (e: SecurityException) {
            Log.w(LOG_TAG, "Could not persist permission for $uri", e)
        }
        Reseam.bundle(bundle).settings.setString(key, uri.toString())
    }
}

private const val FOLDER_ACCESS =
    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

internal fun pickFolder(
    activity: Activity,
    bundle: String,
    key: String,
    onPicked: Runnable? = null,
) =
    requestDocument(
        activity,
        Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
            .addFlags(FOLDER_ACCESS or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION),
        PickFolder(bundle, key),
        onPicked,
    )

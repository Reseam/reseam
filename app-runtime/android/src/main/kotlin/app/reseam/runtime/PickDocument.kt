// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.util.Log

internal data class StoreDocument(val bundle: String, val key: String, val access: Int) :
    DocumentAction {
    override fun complete(activity: Activity, uri: Uri) {
        try {
            activity.contentResolver.takePersistableUriPermission(uri, access)
        } catch (e: SecurityException) {
            Log.w(LOG_TAG, "Could not persist permission for $uri", e)
        }
        Reseam.bundle(bundle).settings.setString(key, uri.toString())
    }
}

private const val FOLDER_ACCESS =
    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

private const val FILE_ACCESS = Intent.FLAG_GRANT_READ_URI_PERMISSION

internal fun pickFolder(activity: Activity, bundle: String, key: String, onPicked: Runnable) =
    requestDocument(
        activity,
        Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
            .addFlags(FOLDER_ACCESS or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION),
        StoreDocument(bundle, key, FOLDER_ACCESS),
    ) {
        onPicked.run()
    }

internal fun pickFile(
    activity: Activity,
    bundle: String,
    key: String,
    mimeTypes: List<String>,
    onPicked: Runnable,
) =
    requestDocument(
        activity,
        openDocument(mimeTypes).addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION),
        StoreDocument(bundle, key, FILE_ACCESS),
    ) {
        onPicked.run()
    }

internal fun openDocument(mimeTypes: List<String>): Intent =
    Intent(Intent.ACTION_OPEN_DOCUMENT)
        .addCategory(Intent.CATEGORY_OPENABLE)
        .addFlags(FILE_ACCESS)
        .setType(mimeTypes.singleOrNull() ?: "*/*")
        .apply {
            if (mimeTypes.size > 1) putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes.toTypedArray())
        }

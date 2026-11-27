// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import kotlin.concurrent.thread

/** Only the folder's name is recognizable to a user in a document tree URI. */
internal fun folderName(value: String): String {
    if (!value.startsWith("content://")) return value
    val last = Uri.parse(value).lastPathSegment ?: return value
    return last.substringAfterLast(':').ifEmpty { value }
}

/**
 * Hands [show] the name the provider of a picked file gives it, or its address when it gives none.
 * Providers can be slow to answer, so the lookup runs off the main thread.
 */
internal fun fileName(context: Context, value: String, show: (String) -> Unit) {
    if (!value.startsWith("content://")) return show(value)
    val main = Handler(Looper.getMainLooper())
    thread(name = "reseam-file-name") {
        val name =
            try {
                context.contentResolver
                    .query(
                        Uri.parse(value),
                        arrayOf(OpenableColumns.DISPLAY_NAME),
                        null,
                        null,
                        null,
                    )
                    ?.use { if (it.moveToFirst()) it.getString(0) else null }
            } catch (_: SecurityException) {
                null
            }
        main.post { show(name ?: value) }
    }
}

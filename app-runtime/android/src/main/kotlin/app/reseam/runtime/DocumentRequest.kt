// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.app.Fragment
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import java.io.Serializable

/** What happens to the document the user picks. */
internal sealed interface DocumentAction : Serializable {
    fun complete(activity: Activity, uri: Uri)
}

/** A framework fragment receives document results without requiring the host to forward them. */
@Suppress("DEPRECATION")
class DocumentRequest : Fragment() {
    // Not kept across recreation: a rebuilt request completes its action without the callback.
    internal var onCompleted: Runnable? = null

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        fragmentManager!!.beginTransaction().remove(this).commit()
        val uri = data?.data
        if (requestCode != REQUEST_CODE || resultCode != Activity.RESULT_OK || uri == null) return
        (arguments!!.getSerializable(ACTION) as DocumentAction).complete(activity!!, uri)
        onCompleted?.run()
    }
}

private const val TAG = "ReseamDocument"
private const val ACTION = "action"
private const val REQUEST_CODE = 0x57C4

/**
 * Starts [intent] over [activity]; the picked document completes [action], then [onCompleted] runs.
 * Neither runs when the user cancels.
 */
@Suppress("DEPRECATION")
internal fun requestDocument(
    activity: Activity,
    intent: Intent,
    action: DocumentAction,
    onCompleted: Runnable? = null,
) {
    val request =
        DocumentRequest().apply {
            this.onCompleted = onCompleted
            arguments = Bundle().apply { putSerializable(ACTION, action) }
        }
    activity.fragmentManager!!.beginTransaction().add(request, TAG).commitNow()
    request.startActivityForResult(intent, REQUEST_CODE)
}

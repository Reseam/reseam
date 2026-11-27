// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import org.json.JSONArray
import org.json.JSONObject

internal fun JSONObject.optionalString(key: String): String? =
    if (isNull(key)) null else getString(key)

internal fun JSONObject.text(key: String): DisplayText {
    val text = getJSONObject(key)
    return if (text.has("text")) Verbatim(text.getString("text"))
    else TextRef(text.getString("table"), text.getString("name"))
}

internal fun JSONObject.optionalText(key: String): DisplayText? =
    if (isNull(key)) null else text(key)

internal fun JSONObject.optionalObject(key: String): JSONObject? =
    if (isNull(key)) null else getJSONObject(key)

internal fun JSONArray.objects(): List<JSONObject> = List(length()) { getJSONObject(it) }

internal fun JSONArray.strings(): List<String> = List(length()) { getString(it) }

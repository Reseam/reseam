// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import org.json.JSONObject

class Typography internal constructor(json: JSONObject) {
    @JvmField val headlineSmall = TypeStyle(json.getJSONObject("headlineSmall"))
    @JvmField val titleLarge = TypeStyle(json.getJSONObject("titleLarge"))
    @JvmField val titleMedium = TypeStyle(json.getJSONObject("titleMedium"))
    @JvmField val bodyLarge = TypeStyle(json.getJSONObject("bodyLarge"))
    @JvmField val bodyMedium = TypeStyle(json.getJSONObject("bodyMedium"))
    @JvmField val labelLarge = TypeStyle(json.getJSONObject("labelLarge"))
    @JvmField val labelMedium = TypeStyle(json.getJSONObject("labelMedium"))
}

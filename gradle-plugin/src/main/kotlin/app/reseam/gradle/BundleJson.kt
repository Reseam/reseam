// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlinx.serialization.json.Json

/** Reads the JSON the CLI and tools print, which may grow fields the build does not read. */
internal val bundleJson = Json { ignoreUnknownKeys = true }

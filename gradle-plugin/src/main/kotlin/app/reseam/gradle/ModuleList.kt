// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlinx.serialization.Serializable

/**
 * The modules a bundle's payload is built from, staged beside it.
 * `crates/patcher/src/bundle/modules.rs` reads it for the patches' fingerprints; both sides check
 * `crates/patcher/tests/fixtures/bundle-modules.json`.
 */
internal const val MODULE_LIST = "modules.json"

/**
 * A module: the staged files it makes and the modules it uses. [source] is the SHA-256 of what
 * files built from every module that uses them are made from besides those modules.
 */
@Serializable
internal data class ModuleListEntry(
    val name: String,
    val files: List<String>,
    val uses: List<String> = emptyList(),
    val source: String? = null,
)

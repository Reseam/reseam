// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The patch index a patch jar carries. `crates/patcher/src/bundle/index.rs` reads it; both sides
 * check `crates/patcher/tests/fixtures/patch-index.json`.
 */
internal const val PATCH_INDEX = "META-INF/reseam/patches.json"

@Serializable
internal enum class MemberKind {
    @SerialName("field") FIELD,
    @SerialName("method") METHOD,
}

@Serializable
internal data class PatchDeclaration(
    @SerialName("class") val className: String,
    val owner: String,
    val member: String,
    val kind: MemberKind,
    val id: String,
)

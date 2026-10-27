// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.io.Serializable

internal data class PublishedBundle(val index: String, val version: String, val signer: String?) :
    Serializable

/** Bundles whose patches this module may reference through generated Kotlin declarations. */
abstract class ReseamPatchesExtension {
    internal val published = mutableListOf<PublishedBundle>()
    internal val local = mutableListOf<File>()

    fun bundle(index: String, version: String, signer: String? = null) {
        published += PublishedBundle(index, version, signer)
    }

    fun bundle(file: File) {
        local += file
    }
}

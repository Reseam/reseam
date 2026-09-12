// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.provider.Provider

/** The engine CLI: `RESEAM_BIN`, `reseam.bin`, the release build of an engine checkout, or `reseam` on the path. */
internal fun Project.reseamBinary(): Provider<String> =
    providers.environmentVariable("RESEAM_BIN")
        .orElse(providers.gradleProperty("reseam.bin"))
        .orElse(
            providers.environmentVariable("RESEAM_WORKSPACE")
                .orElse(providers.gradleProperty("reseam.workspace"))
                .map { "$it/target/release/reseam" },
        )
        .orElse("reseam")

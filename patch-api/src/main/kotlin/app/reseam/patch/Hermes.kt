// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.hermesVersion
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.run.RunJournal

/**
 * The base APK's Hermes bytecode in `assets/index.android.bundle`, opened on first use and written
 * when the patch run finishes. Missing bundles, non-Hermes files and unsupported bytecode versions
 * fail the patch. Version 98 is currently supported.
 */
class HermesScope internal constructor() {
    /** Opens the bundle and returns its bytecode version. */
    val version: Int
        get() = hermesVersion().toInt()

    /**
     * Declares, for the rest of the run, the app modules JavaScript extensions import. An
     * extension's bare specifiers, such as `react` or `app:screens/Home.tsx`, name app modules;
     * each resolves to the Metro module declared here when the extension links. Patches whose
     * extensions import app modules depend on the patch that declares them.
     *
     * ```kotlin
     * val appModules = patch {
     *     execute {
     *         hermes.modules {
     *             module("react", function { name("isValidElement"); paramCount(1) }.module)
     *             prefix("app:") { path -> function { strings(path); paramCount(7) }.module }
     *         }
     *     }
     * }
     * ```
     */
    fun modules(block: AppModules.() -> Unit) = ActiveRuntime.current.run.appModules.block()
}

/** The app modules extensions import, by specifier, declared for the rest of the run. */
class AppModules internal constructor(private val journal: RunJournal) {
    private val modules = mutableMapOf<String, MetroModule>()
    private val prefixes = mutableMapOf<String, (String) -> MetroModule>()

    /** Resolves the import [specifier] to [module]. */
    fun module(specifier: String, module: MetroModule) {
        val previous = modules.putIfAbsent(specifier, module)
        require(previous == null || previous === module) { "$specifier is declared twice" }
        if (previous == null) journal.record { modules.remove(specifier) }
    }

    /**
     * Resolves imports that start with [prefix], and have no [module] declaration, by the rest of
     * the specifier.
     */
    fun prefix(prefix: String, module: (path: String) -> MetroModule) {
        val previous = prefixes.putIfAbsent(prefix, module)
        require(previous == null || previous === module) { "$prefix is declared twice" }
        if (previous == null) journal.record { prefixes.remove(prefix) }
    }

    internal fun resolve(specifier: String): MetroModule =
        modules.getOrPut(specifier) {
            val module =
                prefixes.entries
                    .firstOrNull { (prefix, _) -> specifier.startsWith(prefix) }
                    ?.let { (prefix, module) -> module(specifier.removePrefix(prefix)) }
                    ?: error(
                        "No app module is declared for the import $specifier; depend on the patch " +
                            "that declares the app's modules"
                    )
            journal.record { modules.remove(specifier) }
            module
        }
}

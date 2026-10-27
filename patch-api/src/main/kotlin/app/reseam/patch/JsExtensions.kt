// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.hermesModuleImports
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.HermesImport
import app.reseam.patch.types.HermesModule

/**
 * A JavaScript extension of the bundle, named by its workspace module: for
 * `apps/example/extensions/features`, `example-features`. The Gradle plugin generates one binding
 * per extension from its entry module's exports; it is never written by hand.
 *
 * The extension's sources live in `src/main/js`: the entry module `index.ts`, `index.tsx`,
 * `index.js` or `index.jsx`, bundled with the modules it imports by relative path. Bare specifiers
 * name app modules, which [HermesScope.modules] declares; JSX uses the app's `react/jsx-runtime`.
 * The extension's code first runs when a hook first calls one of its exports.
 */
open class ExtJsModule(val name: String) {
    internal open val source: HermesModule
        get() =
            HermesModule.Bundle(
                name,
                hermesModuleImports(name).map { specifier ->
                    HermesImport(
                        specifier,
                        ActiveRuntime.current.run.appModules.resolve(specifier).handle,
                    )
                },
            )

    /** Declares a function the extension exports, named after the property, or [name]. */
    fun export(name: String? = null): MemberDelegate<JsExport> =
        MemberDelegate(name) { JsExport(this, it) }

    override fun toString() = name
}

/** A function an extension exports; hooks call it through [JsCodeScope.call]. */
class JsExport internal constructor(val module: ExtJsModule, val name: String) {
    override fun toString() = "$module.$name"
}

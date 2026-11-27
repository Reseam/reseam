// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.settings.SettingsContribution
import app.reseam.patch.settings.SettingsHost
import app.reseam.patch.settings.StringRef

interface ReseamPatch {
    /**
     * Display name; need not be unique. Identity comes from the public declaration. Null means an
     * internal dependency, never listed.
     */
    val name: StringRef?
    val hidden: Boolean
        get() = name == null

    val description: StringRef?
        get() = null

    val dependencies: List<ReseamPatch>
        get() = emptyList()

    val compatibleWith: List<CompatiblePackage>
        get() = emptyList()

    /**
     * A patch with no declared package works with every app, so it is off unless its author enables
     * it by default.
     */
    val universal: Boolean
        get() = compatibleWith.isEmpty()

    val enabled: Boolean
        get() = !hidden && !universal

    val options: List<Option<*>>
        get() = emptyList()

    fun execute(ctx: PatchRuntime)

    /**
     * Runs once after execution succeeds, after dependent finalizers, including for leaf patches.
     */
    fun afterDependents(ctx: PatchRuntime) {}

    /**
     * Whether [afterDependents] does work. The engine calls it only when this is set, and reports
     * the patch's result as soon as it executes when it is not.
     */
    val finalizes: Boolean
        get() = true

    /**
     * Runs after the engine wrote the APK files and before it signs them, for patches that
     * [rewritesOutput]. Patches that do run dependencies first.
     */
    fun afterWrite(ctx: OutputRuntime) {}

    /**
     * Whether [afterWrite] does work. A mount build leaves such a patch out, since the engine
     * cannot tell what the replaced files hold.
     */
    val rewritesOutput: Boolean
        get() = false
}

/**
 * A package the patch applies to. An APK matches when its version name is in [versions] or its
 * version code is in [versionCodes]; with both empty, every version matches.
 */
data class CompatiblePackage
@JvmOverloads
constructor(
    val name: String,
    val versions: List<String> = emptyList(),
    /** `android:versionCode` values. */
    val versionCodes: List<Int> = emptyList(),
)

/**
 * A patch in another bundle, for `dependsOn`. The Gradle plugin generates one per patch of every
 * bundle a module declares; it is never written by hand.
 */
class ExternalPatch(val bundle: String, val id: String) : ReseamPatch {
    override val name: StringRef? = null

    override fun execute(ctx: PatchRuntime) = error("$this belongs to another bundle")

    override fun toString() = "$bundle/$id"
}

internal class BuiltPatch(
    override val name: StringRef?,
    override val hidden: Boolean,
    override val description: StringRef?,
    override val dependencies: List<ReseamPatch>,
    override val compatibleWith: List<CompatiblePackage>,
    enabledByDefault: Boolean?,
    override val options: List<Option<*>>,
    private val settings: List<Pair<SettingsHost, PatchRuntime.() -> List<SettingsContribution>>>,
    private val execute: (PatchRuntime.() -> Unit)?,
    private val finalize: (PatchRuntime.() -> Unit)?,
    private val rewrite: (OutputRuntime.() -> Unit)?,
) : ReseamPatch {
    override val enabled = !hidden && (enabledByDefault ?: !universal)
    override val finalizes = finalize != null
    override val rewritesOutput = rewrite != null

    override fun execute(ctx: PatchRuntime) {
        execute?.invoke(ctx)
        for ((host, blocks) in settings.groupBy({ it.first }, { it.second })) {
            host.register(this, blocks.flatMap { it(ctx) })
        }
    }

    override fun afterDependents(ctx: PatchRuntime) {
        finalize?.invoke(ctx)
    }

    override fun afterWrite(ctx: OutputRuntime) {
        rewrite?.invoke(ctx)
    }

    override fun toString() = name?.toString() ?: "internal patch"
}

// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.settings.SettingsHost
import app.reseam.patch.settings.SettingsSection
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

operator fun String.invoke(vararg versions: String) = CompatiblePackage(this, versions.toList())

/** A version known by its name and code; an APK matches when either one does. */
operator fun String.invoke(versionName: String, versionCode: Int) =
    CompatiblePackage(this, listOf(versionName), listOf(versionCode))

/**
 * A patch in another bundle, for `dependsOn`. The Gradle plugin generates one per patch of every
 * bundle a module declares; it is never written by hand.
 */
class ExternalPatch(val bundle: String, val id: String) : ReseamPatch {
    override val name: StringRef? = null

    override fun execute(ctx: PatchRuntime) = error("$this belongs to another bundle")

    override fun toString() = "$bundle/$id"
}

/**
 * A patch users see as [name], a string of the module's `src/main/strings` that the bundle's
 * catalog carries in every language the module translates it to.
 */
fun patch(name: StringRef, block: PatchBuilder.() -> Unit): ReseamPatch =
    PatchBuilder(name).apply(block).build()

fun patch(block: PatchBuilder.() -> Unit): ReseamPatch = PatchBuilder(null).apply(block).build()

abstract class PatchDeclaration internal constructor() {
    internal val compatibility = mutableListOf<CompatiblePackage>()
    internal val dependencies = mutableListOf<ReseamPatch>()

    fun compatibleWith(vararg packageNames: String) {
        compatibility += packageNames.map { CompatiblePackage(it) }
    }

    fun compatibleWith(vararg packages: CompatiblePackage) {
        compatibility += packages
    }

    fun dependsOn(vararg patches: ReseamPatch) {
        dependencies += patches
    }
}

class PatchBuilder internal constructor(private val name: StringRef?) : PatchDeclaration() {
    private var description: StringRef? = null
    private var hidden = name == null
    private var enabledByDefault: Boolean? = null
    private val options = mutableListOf<Option<*>>()
    private val settings =
        mutableListOf<Pair<SettingsHost, PatchRuntime.() -> List<SettingsSection>>>()
    private var executeBlock: (PatchRuntime.() -> Unit)? = null
    private var afterDependentsBlock: (PatchRuntime.() -> Unit)? = null
    private var afterWriteBlock: (OutputRuntime.() -> Unit)? = null

    fun description(text: StringRef) {
        description = text
    }

    fun enabledByDefault(value: Boolean) {
        enabledByDefault = value
    }

    /** Keeps a named patch off the user-facing lists. */
    fun hidden() {
        hidden = true
    }

    fun stringOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: String? = null,
        validValues: List<String>? = null,
        required: Boolean = false,
    ) = StringOption(key, title, description, required, default, validValues).also { options += it }

    fun boolOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Boolean? = null,
        required: Boolean = false,
    ) = BoolOption(key, title, description, required, default).also { options += it }

    fun intOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Long? = null,
        required: Boolean = false,
    ) = IntOption(key, title, description, required, default).also { options += it }

    fun floatOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: Double? = null,
        required: Boolean = false,
    ) = FloatOption(key, title, description, required, default).also { options += it }

    fun stringListOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        default: List<String>? = null,
        required: Boolean = false,
    ) = StringListOption(key, title, description, required, default).also { options += it }

    fun pathOption(
        key: String,
        title: StringRef,
        description: StringRef? = null,
        required: Boolean = false,
    ) = PathOption(key, title, description, required).also { options += it }

    /** Registers settings with `host`, which becomes a dependency. */
    fun settings(host: SettingsHost, vararg sections: SettingsSection) {
        val declared = sections.toList()
        settings(host) { declared }
    }

    /** Registers settings computed after `execute`, so they can depend on options. */
    fun settings(host: SettingsHost, sections: PatchRuntime.() -> List<SettingsSection>) {
        settings += host to sections
        if (host !in dependencies) dependencies += host
    }

    fun execute(block: PatchRuntime.() -> Unit) {
        executeBlock = block
    }

    fun afterDependents(block: PatchRuntime.() -> Unit) {
        afterDependentsBlock = block
    }

    /**
     * Works on the APK files the engine wrote, before it signs them. The engine has released the
     * patched app by then: [OutputRuntime] reaches only the files, the bundle and the options.
     */
    fun afterWrite(block: OutputRuntime.() -> Unit) {
        afterWriteBlock = block
    }

    internal fun build(): ReseamPatch {
        val builder = this
        val contributions = settings.toList()
        val execute = executeBlock
        val finalize = afterDependentsBlock
        val rewrite = afterWriteBlock
        return object : ReseamPatch {
            override val name = builder.name
            override val hidden = builder.hidden
            override val description = builder.description
            override val dependencies = builder.dependencies.toList()
            override val compatibleWith = builder.compatibility.toList()
            override val enabled = !builder.hidden && (builder.enabledByDefault ?: !universal)
            override val options = builder.options.toList()
            override val finalizes = finalize != null
            override val rewritesOutput = rewrite != null

            override fun execute(ctx: PatchRuntime) {
                execute?.invoke(ctx)
                for ((host, blocks) in contributions.groupBy({ it.first }, { it.second })) {
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
    }
}

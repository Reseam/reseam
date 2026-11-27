// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.recordingResolutions
import app.reseam.patch.run.MethodEntries
import app.reseam.patch.run.RunCache
import app.reseam.patch.run.RunJournal
import app.reseam.patch.settings.SettingsContribution
import app.reseam.patch.settings.SettingsHost
import java.util.IdentityHashMap

internal class PatchRun {
    private val journal = RunJournal()
    val entries = MethodEntries(journal)
    val cache = RunCache()
    val appModules = AppModules(journal)
    private val settings =
        IdentityHashMap<SettingsHost, MutableList<Pair<ReseamPatch, List<SettingsContribution>>>>()
    private val settingsOwners = mutableMapOf<String, SettingsHost>()
    private val iconKeys = IdentityHashMap<SettingsHost, Set<String>>()
    private val lists = IdentityHashMap<RunList<*>, MutableList<Any?>>()

    /**
     * Whether the engine records how this run's targets resolve, for a check of a bundle's targets.
     */
    val recordsResolutions: Boolean by lazy { recordingResolutions() }

    fun add(list: RunList<*>, value: Any?) {
        val values = lists.getOrPut(list) { mutableListOf() }
        values += value
        journal.record { values.removeAt(values.lastIndex) }
    }

    @Suppress("UNCHECKED_CAST")
    fun <T> values(list: RunList<T>): List<T> = lists[list].orEmpty().toList() as List<T>

    fun register(host: SettingsHost, patch: ReseamPatch, sections: List<SettingsContribution>) {
        keepSettings(host)
        val registered = settings.getOrPut(host) { mutableListOf() }
        val index = registered.indexOfFirst { it.first === patch }
        if (index < 0) registered += patch to sections else registered[index] = patch to sections
    }

    fun sections(host: SettingsHost): List<SettingsContribution> =
        settings[host].orEmpty().flatMap { it.second }

    fun clear(host: SettingsHost) {
        keepSettings(host)
        settings.remove(host)
        iconKeys.remove(host)
    }

    fun showIcons(host: SettingsHost, keys: Set<String>) {
        val previous = iconKeys.put(host, keys)
        journal.record {
            if (previous == null) iconKeys.remove(host) else iconKeys[host] = previous
        }
    }

    fun iconKeys(host: SettingsHost): Set<String>? = iconKeys[host]

    /** Records that [host] writes [bundle]'s settings, which one host per app may do. */
    fun claimSettings(bundle: String, host: SettingsHost) {
        val owner = settingsOwners.putIfAbsent(bundle, host)
        require(owner == null || owner === host) {
            "$bundle has two settings hosts for this app: $owner and $host"
        }
        if (owner == null) journal.record { settingsOwners.remove(bundle) }
    }

    private fun keepSettings(host: SettingsHost) {
        val previous = settings[host]?.toList()
        journal.record {
            if (previous == null) settings.remove(host)
            else settings[host] = previous.toMutableList()
        }
    }

    @JvmName("openSavepoint") internal fun openSavepoint(savepoint: Int) = journal.open(savepoint)

    @JvmName("releaseSavepoint")
    internal fun releaseSavepoint(savepoint: Int) = journal.release(savepoint)

    @JvmName("rollbackTo") internal fun rollbackTo(savepoint: Int) = journal.rollback(savepoint)
}

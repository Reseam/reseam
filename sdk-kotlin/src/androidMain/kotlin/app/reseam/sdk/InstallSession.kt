// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.sdk

import android.content.pm.PackageInstaller
import android.os.Build
import java.io.File

/**
 * Writes [apks] into this install session, each followed by the dex metadata (`.dm`) that has the
 * installer compile the app with its baseline profile. Without it ART runs a patched app uncompiled
 * until the device's idle maintenance. The session is left open for the caller to commit or
 * abandon.
 */
fun PackageInstaller.Session.writeApks(apks: List<File>) {
    for (apk in apks) {
        openWrite(apk.name, 0, apk.length()).use { stream ->
            apk.inputStream().use { it.copyTo(stream) }
            fsync(stream)
        }
        val metadata = dexMetadata(apk.path, Build.VERSION.SDK_INT.toUInt()) ?: continue
        openWrite("${apk.nameWithoutExtension}.dm", 0, metadata.size.toLong()).use { stream ->
            stream.write(metadata)
            fsync(stream)
        }
    }
}

// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

internal fun requireActivePatchContext() {
    check(ctxIsActive()) {
        "This API is only available while a patch is executing."
    }
}

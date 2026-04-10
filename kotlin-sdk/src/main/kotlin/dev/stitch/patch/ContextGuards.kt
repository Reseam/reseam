package dev.stitch.patch

internal fun requireActivePatchContext() {
    check(ctxIsActive()) {
        "This API is only available while a patch is executing."
    }
}

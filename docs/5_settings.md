---
description: Add switches to the patched app so users can turn features on and off.
---

# Settings inside the app

[Options](4_patches.md#options) are chosen once, before patching. Settings live inside the patched app: a Reseam settings screen with switches the user can change at any time, and patched code that checks them.

It takes three parts:

1. **Settings**, declared in Kotlin.
2. **A settings host**, an internal patch that adds the settings screen to the app.
3. **Gates**, patched code that reads a setting when the app runs.

## The runtime

The settings screen and the code that reads settings are Java, in the `shared/settings-runtime` module of the [official patches repository](https://git.reseam.app/reseam/patches/src/branch/main/shared/settings-runtime). Copy that folder into your bundle's `shared/` folder. The engine links it into the app the first time a patch uses a setting.

## Declare settings

```kotlin
object AppSettings {
    val hideAds by toggle("Hide ads", default = true)
    val feedStyle by choice("Feed style", default = "compact", choices = listOf(Choice("compact", "Compact"), Choice("cards", "Cards")))
}
```

`toggle`, `text`, `folder`, and `choice` each take a title, an optional summary, and a default. The storage key comes from the object and property names: `AppSettings.hideAds` is stored as `app_settings.hide_ads`.

> [!WARNING]
> Renaming the object or the property changes the key and resets the setting for every user. Pin it with `key = "..."` before your first release.

## Add the settings screen

```kotlin
val appSettings = settingsHost("example") {
    compatibleWith(EXAMPLE_APP)

    install {
        appEntry { call(SettingsEntry.init, application) }
    }
}
```

`install` runs after every patch that added settings, once the host has written them to `assets/reseam/settings.json`. `SettingsEntry.init`, in an [extension](11_extensions.md), calls `ReseamSettings.init(context)` when the app starts.

`ReseamSettingsScreen.open()` shows the settings over the app's current screen, from any thread. How users get there is up to you, for example a row in the app's own settings that calls it. From a patch, call `ReseamSettingsScreen.open` like any extension method. When the app builds the row from an intent, such as an androidx `Preference` with an `<intent>`, give the intent the action `SETTINGS_OPEN_ACTION` and redirect the call that starts it to `ReseamSettingsScreen.startActivity`.

The settings need no activity of their own. A host that leaves the manifest alone keeps every patch that uses settings in [mount builds](9_app_files.md#mount-builds).

## Register a patch's settings

```kotlin
val hideAds = patch("Hide ads") {
    compatibleWith(EXAMPLE_APP)
    settings(appSettings, section("Ads", AppSettings.hideAds))

    execute {
        gate(AppSettings.hideAds) {
            showAd.alwaysReturn()
        }
    }
}
```

`settings(host, section(...))` adds the section to the screen and makes the host a dependency. Only settings from patches that actually run appear. Inside `gate`, the method returns early whenever the toggle is on. See [Gates](8_changing_code.md#gates).

When a setting depends on an option, register it from a block. It runs after `execute`, so it can read options:

```kotlin
settings(appSettings) {
    listOf(section("Name", AppSettings.appName))
}
```

## Pages

Group sections behind a row on the screen with a page:

```kotlin
val mediaPage = SettingsPage("media", "Media", order = 10)

settings(appSettings, section(mediaPage, "Playback", AppSettings.autoplay))
```

A page can have a `parent` page. Lower `order` comes first. Sections with the same page and title merge into one. Pages without settings from running patches are left out.

Next: [Finding code](6_finding_code.md).

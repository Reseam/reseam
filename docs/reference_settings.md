---
title: "API: Settings"
description: Declaring settings, the settings screen, gates, brands, and icons.
---

# API: Settings

Settings the user changes inside the patched app, and the patched code that reads them. The guide page is [Settings inside the app](11_settings.md). Everything here is in `app.reseam.patch.settings`.

Your [extensions](10_extensions.md) read the same settings through generated objects. Those are listed in [API: App runtime](reference_runtime.md#settings-in-extensions).

## Declaring settings

### `SettingsGroup`

The base of an `object` that declares settings as properties.

```kotlin
object AppSettings : SettingsGroup(ExampleStrings) {
    val hideAds by toggle(default = true)
}
```

| Member | Description |
|---|---|
| `SettingsGroup(strings: StringTable, keyPrefix: String? = null)` | `strings` holds the settings' titles. `keyPrefix` starts every key; it defaults to the object's name in snake case, so `AppSettings` gives `app_settings`. Give two objects the same prefix to split one set of settings without changing their keys. |
| `strings: StringTable` | The table the titles come from. |

A setting's title is the string named after its property in snake case: `hideAds` reads `hide_ads`. Its summary is `hide_ads_summary`, when the table has one. A missing title fails the build.

### Setting functions

Use these as property delegates inside a `SettingsGroup`. `key` defaults to the group's prefix and the property name, so `AppSettings.hideAds` is stored as `app_settings.hide_ads`.

| Function | Description |
|---|---|
| `toggle(default: Boolean, key: String? = null, icon: Vector? = null, covers: List<ToggleSetting> = emptyList())` | An on/off switch. While it is on, each toggle in `covers` reads as on and shows greyed out. Their own values come back when it is turned off. |
| `text(default: String, multiline: Boolean = false, key: String? = null, icon: Vector? = null)` | Text the user types. A `multiline` text holds one entry per line. |
| `folder(default: String, key: String? = null, icon: Vector? = null)` | A folder the user picks. The app gets permission to read and write it. |
| `choice(default: String, choices: List<Choice>, key: String? = null, icon: Vector? = null)` | One of several values. `default` must be one of the choices' values. |

> [!WARNING]
> **Pin your keys before the first release.** If you later rename the object or the property, the key changes, and every user's saved value is lost. Set `key = "..."` on each setting, and you can rename the code freely.

### `SettingDelegate`

What the setting functions return: a property delegate that works out the key from the property name. You never use it directly.

### `Setting`

The base of every setting.

| Member | Description |
|---|---|
| `key: String` | Where the value is stored. Keys only have to be unique within your bundle, since each bundle stores its settings apart. |
| `title: StringRef` | The title on the settings screen. |
| `summary: StringRef?` | The line under the title, if any. |
| `default: T` | The value until the user changes it. |
| `icon: Vector?` | The icon at the start of the row, if any. |

### `ToggleSetting`

An on/off setting. `covers: List<ToggleSetting>` is the toggles it switches on while it is on.

### `TextSetting`

A text setting. `multiline: Boolean` says whether it holds one entry per line.

### `FolderSetting`

A folder setting. Its value is the folder's address, as text.

### `ChoiceSetting`

| Member | Description |
|---|---|
| `choices: List<Choice>` | The values the user picks from. |
| `copy(default: String = this.default, choices: List<Choice> = this.choices): ChoiceSetting` | The same setting, under the same key, with other choices. A patch uses it to offer choices that depend on its options. |

### `Choice`

`Choice(value: String, title: DisplayText)`: the stored `value`, and the `title` users see.

## Text

### `StringTable`

Translated strings, generated from the `strings.xml` files in a settings module's `src/main/strings`. The generated object has one `StringRef` member per string, named in camel case: `feed_style_compact` becomes `ExampleStrings.feedStyleCompact`.

| Member | Description |
|---|---|
| `id: String` | The table's name in the app. |

### `StringRef`

One string of a table, shown in the app's current language.

| Member | Description |
|---|---|
| `table: StringTable` | Its table. |
| `name: String` | Its name in `strings.xml`. |

### `Verbatim`

`Verbatim(text: String)`: text shown as it is in every language, such as a name the user typed into an option.

### `DisplayText`

Text a settings screen shows: a `StringRef` or a `Verbatim`.

## Laying out the screen

### `SettingsSection`

A group of settings under a heading.

| Member | Description |
|---|---|
| `title: StringRef` | The heading. |
| `settings: List<Setting<*>>` | The settings, in order. |
| `page: SettingsPage?` | The page it is on, or null for the main screen. |

### `section`

| Function | Description |
|---|---|
| `section(title: StringRef, vararg settings: Setting<*>): SettingsSection` | A section on the main screen. |
| `section(page: SettingsPage, title: StringRef, vararg settings: Setting<*>): SettingsSection` | A section on a page. Sections with the same page and title merge, so several patches can add to one section. |

### `SettingsPage`

A page of its own, shown as a row on its parent.

| Member | Description |
|---|---|
| `SettingsPage(id: String, title: StringRef, parent: SettingsPage? = null, order: Int = 0, summary: StringRef? = null, icon: Vector? = null)` | `parent` nests it in another page. Lower `order` comes first among its siblings. |
| `id`, `title`, `parent`, `order`, `summary`, `icon` | As above. |

Only pages with settings from patches that ran appear, along with their parents.

## The settings host

### `settingsHost`

`settingsHost(appId: String, block: SettingsHostBuilder.() -> Unit): SettingsHost` declares the internal patch that adds your bundle's settings screen to one app. Each bundle has one host per app. `appId` is a short name for the app, used in the host patch's description: `settingsHost("example")` is described as "Settings for example".

### `SettingsHostBuilder`

The receiver of `settingsHost { }`. It has `compatibleWith` and `dependsOn` from [`PatchDeclaration`](reference.md#patchdeclaration), plus:

| Member | Description |
|---|---|
| `settings(vararg sections: SettingsSection)` | Sections the host itself owns, for settings that belong to no single patch. They lead the screen. |
| `strings(vararg tables: StringTable)` | String tables your extensions read. The tables your settings use ship anyway. |
| `brand(brand: Brand)` | How the settings screen and your branded components look. |
| `about(about: About)` | Links and buttons on the screen's about card. |
| `install(block: PatchRuntime.() -> Unit)` | Connects the screen to the app, typically a row in the app's own settings that calls `openSettings()`. It runs once every patch has registered its settings. |

### `SettingsHost`

What `settingsHost` returns: a [`ReseamPatch`](reference.md#reseampatch), with all its members, that you pass to `PatchBuilder.settings`.

| Member | Description |
|---|---|
| `appId: String` | The app's short name. |
| `register(patch: ReseamPatch, sections: List<SettingsSection>)` | Adds a patch's sections. `PatchBuilder.settings` calls it for you. |

What the host does when it runs: it writes your bundle's settings, string tables, and brand into the app under `assets/reseam/<bundle>/`, adds the app runtime, and starts it first in the `Application`'s `onCreate`. The app needs an `Application` class.

## Gates

### `gate`

`gate(setting: ToggleSetting, block: GateScope.() -> Unit)` applies the hooks written in the block only while the toggle is on. The hooked code reads the toggle each time it runs.

A Hermes hook in a gate reads the toggle once per process, through the React Native module that [`addReactPackage`](#the-app-runtime-in-code-blocks) adds.

### `GateScope`

The receiver of `gate { }`. Each member works like the same hook outside a gate.

| Member | Description |
|---|---|
| `MethodTarget.before { }`, `MethodTarget.after { }` | Code at the start of a method, or before its returns. |
| `PointTarget.before { }`, `PointTarget.after { }` | Code around an instruction. |
| `PointTarget.skip()` | Skips the call at the point. Its result must be unused. |
| `MethodTarget.alwaysReturn()`, `alwaysReturn(value: Boolean / Int / Long / String)`, `alwaysReturnNull()` | Returns at once with this value. |
| `FunctionTarget.before { }`, `after { }`, `replace { }` | [Hermes hooks](reference_hermes.md#hooks). |
| `FunctionTarget.alwaysReturn()`, `alwaysReturn(value: Boolean / Int / String)`, `alwaysReturnNull()` | Hermes constant returns. |
| `FunctionTarget.setArgument(index: Int, path: String, value: Boolean)` | Calls the JavaScript function with a copy of argument `index` whose property at `path` is `value`. `path` names nested properties with dots, such as `options.compact`. Each object on the way is copied, so the caller's object doesn't change. |
| `setting: ToggleSetting` | The gate's toggle. |

### `whenEnabled`

| Function | Description |
|---|---|
| `CodeScope.whenEnabled(setting: ToggleSetting, block: CodeScope.() -> Unit): Otherwise` | Inside a code block: runs the block in the app when the toggle is on. |
| `JsCodeScope.whenEnabled(setting: ToggleSetting, block: JsCodeScope.() -> Unit): JsOtherwise` | The same, in a Hermes code block. |

### Reading settings in code blocks

`valueOf` reads a setting's current value in the app. Before the app's `Application.onCreate` has run, such as in a content provider, it reads the default.

| Function | Description |
|---|---|
| `CodeScope.valueOf(setting: ToggleSetting): ValueRef` | A `boolean`. |
| `CodeScope.valueOf(setting: TextSetting): ValueRef` | A `String`. |
| `CodeScope.valueOf(setting: FolderSetting): ValueRef` | The folder's address, as a `String`. |
| `CodeScope.valueOf(setting: ChoiceSetting): ValueRef` | The chosen value, as a `String`. A stored value the setting no longer lists reads as the default. |
| `JsCodeScope.valueOf(setting: ToggleSetting): JsValue` | A toggle's value, in a Hermes code block. |

### The app runtime in code blocks

| Member | Description |
|---|---|
| `CodeScope.openSettings(): ValueRef` | Opens your bundle's settings over the app's current screen. It works from any thread. |
| `XmlElement.opensSettings()` | Makes an `<intent>` element, such as the one an androidx `Preference` starts, open your settings. Also redirect the row's `startActivity` call to `Reseam.startActivity`, which recognizes the intent. |
| `CodeScope.addReactPackage(packages: ValueRef): ValueRef` | Adds the React Native package that Hermes gates read settings through to `packages`, the app's package list. It is added once, however many bundles call it. |

### `Reseam`

The app runtime's entry class, as an [`ExtClass`](reference_code.md#extclass), for patches to call.

| Member | Description |
|---|---|
| `startActivity: ExtMethod` | `startActivity(Context, Intent)`: opens your settings for an intent `opensSettings` set up, and starts any other intent as usual. Redirect a settings row's `startActivity` call to it. |

## Branding

### `Brand`

How your settings screen and [branded components](reference_runtime.md#branded-components) look. The screen follows the system's light or dark theme, and before Android 10, which has none, the app's own, until the user picks one with the button at the end of its top bar. A brand has a color scheme for each.

| Member | Description |
|---|---|
| `Brand(name: String, light: ColorScheme, dark: ColorScheme, logo: Logo? = null, font: Font? = null, typography: Typography = Typography())` | `name` shows on the about card and can't be blank. |
| `name`, `light`, `dark`, `logo`, `font`, `typography` | As above. |
| `Brand.DEFAULT` | Material 3's baseline colors and type, in the system font, without a logo. A host without `brand` uses it. |

### `ColorScheme`

The Material 3 color roles the app runtime paints with. Every role is required: `primary`, `onPrimary`, `primaryContainer`, `onPrimaryContainer`, `secondaryContainer`, `onSecondaryContainer`, `surface`, `surfaceContainerLow`, `surfaceContainer`, `surfaceContainerHigh`, `surfaceContainerHighest`, `onSurface`, `onSurfaceVariant`, `outline`, `outlineVariant`, `error`, `onError`, `errorContainer`, `onErrorContainer`, and `scrim`.

### `Color`

An sRGB color. `Color.hex(value: String): Color` reads `#rgb`, `#rrggbb`, or `#aarrggbb`, with alpha first.

### `Typography`

The Material 3 type roles: `headlineSmall`, `titleLarge`, `titleMedium`, `bodyLarge`, `bodyMedium`, `labelLarge`, and `labelMedium`. Each defaults to Material 3's size and weight, so override only the roles you want different: `Typography(titleLarge = TextStyle(22f, 28f, 600))`.

### `TextStyle`

`TextStyle(size: Float, lineHeight: Float, weight: Int, tracking: Float = 0f)`: size, line height, and letter spacing in sp, and a weight from 1 to 1000.

### `Font`

`Font(file: ByteArray)`: a TrueType or OpenType file, read from your patch module's resources. A variable font with a `wght` axis renders every weight the type roles name.

### `Logo`

`Logo(light: Vector, dark: Vector = light)`: your mark for each theme.

### `Vector`

A vector image, such as an icon or a logo, drawn at any size.

`Vector.svg(source: String): Vector` reads an SVG when your patches load. The `svg` element needs a `viewBox`, or a `width` and `height`, with a positive size. These elements draw: `path`, `circle`, `ellipse`, `rect`, `line`, `polyline`, and `polygon`, inside any number of `g`. Paint comes from `fill`, `stroke`, `stroke-width`, `stroke-linecap`, `stroke-linejoin`, and `fill-rule`, inherited from enclosing elements. `currentColor` takes the color the screen draws icons in, so line icons such as [Lucide](https://lucide.dev) work as they are. A `#rgb` or `#rrggbb` color stays fixed, and `none` paints nothing. Anything else, such as transforms, `style`, gradients, or text, fails the build.

```kotlin
private object Icons

val megaphone = Vector.svg(Icons::class.java.getResource("/icons/megaphone.svg")!!.readText())
```

### `About`

`About(links: List<Link> = emptyList(), actions: List<Link> = emptyList())`: where people reach you, on the settings screen's about card. `links` show as icon buttons under your name, and `actions` as buttons below them.

### `Link`

`Link(label: DisplayText, url: String, icon: Vector)`: one link. `url` must be an `https://` address. An icon button shows no label, but screen readers read it aloud.

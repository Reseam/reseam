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

A setting's title is the string named after its property in snake case: `hideAds` reads `hide_ads`. Its summary is `hide_ads_summary`, when the table has one. A toggle's row shows `hide_ads_summary_on` or `hide_ads_summary_off` for its value, and `hide_ads_summary` when that one is missing. A missing title fails the build.

### Setting functions

Use these as property delegates inside a `SettingsGroup`. `key` defaults to the group's prefix and the property name, so `AppSettings.hideAds` is stored as `app_settings.hide_ads`.

| Function | Description |
|---|---|
| `toggle(default: Boolean, key: String? = null, icon: Vector? = null, covers: List<ToggleSetting> = emptyList(), takesEffect: TakesEffect = TakesEffect.ON_RESTART)` | An on/off switch. While it is on, each toggle in `covers` reads as on and shows greyed out. Their own values come back when it is turned off. |
| `text(default: String, multiline: Boolean = false, key: String? = null, icon: Vector? = null, takesEffect: TakesEffect = TakesEffect.ON_RESTART, showValue: Boolean = true)` | Text the user types. A `multiline` text holds one entry per line. The row shows the value, or its summary while the value is empty. With `showValue = false` it always shows the summary. |
| `number(default: Double, slider: Slider, key: String? = null, icon: Vector? = null, takesEffect: TakesEffect = TakesEffect.ON_RESTART)` | A number the user picks on a [`Slider`](#slider). `default` must be on the slider. |
| `range(default: ClosedFloatingPointRange<Double>, slider: Slider, key: String? = null, icon: Vector? = null, takesEffect: TakesEffect = TakesEffect.ON_RESTART)` | A start and an end the user picks with the two thumbs of a [`Slider`](#slider), such as `0.2..0.8`. Both must be on the slider. |
| `color(default: Long, opacity: Boolean = false, choice: ChoiceSetting? = null, key: String? = null, icon: Vector? = null, takesEffect: TakesEffect = TakesEffect.ON_RESTART)` | A color the user picks, with `default` as ARGB, such as `0xFFFF0033`. Without `opacity` the color is always opaque, so `default` needs alpha `FF`. A `choice` is picked above the color in the same dialog, such as what a colored segment does, and gets no row of its own. |
| `folder(default: String, key: String? = null, icon: Vector? = null, takesEffect: TakesEffect = TakesEffect.ON_RESTART)` | A folder the user picks. The app gets permission to read and write it. |
| `file(default: String = "", mimeTypes: List<String> = listOf("*/*"), key: String? = null, icon: Vector? = null, takesEffect: TakesEffect = TakesEffect.ON_RESTART)` | A file the user picks, of one of `mimeTypes`. The app gets permission to read it. |
| `choice(default: String, choices: List<Choice>, key: String? = null, icon: Vector? = null, takesEffect: TakesEffect = TakesEffect.ON_RESTART, showValue: Boolean = true)` | One of several values. `default` must be one of the choices' values. With `showValue = false` the row shows its summary instead of the chosen title. |
| `action(key: String? = null, icon: Vector? = null)` | A row that runs code when the user taps it, such as copying something or opening a page. It stores nothing. A patch gives it its code with [`onAction`](#onaction). Its title string is optional, for a row that shows only its summary and a view a patch draws with [`onView`](#onview). |
| `note(key: String? = null, icon: Vector? = null)` | A row that only shows its summary, such as notes on the settings around it. It stores nothing. Its title string is optional. A note with neither a title nor a summary shows only the view a patch draws with [`onView`](#onview). Without a `_summary` string, a patch can give it one with [`onSummary`](#onsummary). |
| `noteOf(key: String, title: StringRef, summary: StringRef?)` | A note made while patching, outside any group, for rows that depend on the patch run, such as which patches it applied. Register it in a section from `afterDependents`, once the patches it describes have run. Its `key` must differ from every declared setting's. |

Summaries, titles and choice titles show bold, italic and other styling when their string has it. See [Styled strings](11_settings.md#styled-strings).

> [!WARNING]
> **Pin your keys before the first release.** If you later rename the object or the property, the key changes, and every user's saved value is lost. Set `key = "..."` on each setting, and you can rename the code freely.

### `SettingDelegate`

What the setting functions return: a property delegate that works out the key from the property name. You never use it directly.

### `Setting`

The base of every setting.

| Member | Description |
|---|---|
| `key: String` | Where the value is stored. Keys only have to be unique within your bundle, since each bundle stores its settings apart. |
| `title: StringRef?` | The title on the settings screen. Only an action or a note can lack one. |
| `summary: StringRef?` | The line under the title, if any. |
| `default: T` | The value until the user changes it. An action's is `Unit`. |
| `icon: Vector?` | The icon at the start of the row, if any. |
| `takesEffect: TakesEffect` | When the app picks up a change. An action's is `IMMEDIATELY`. |

### `TakesEffect`

When the app picks up a change to a setting. See [When a change applies](11_settings.md#when-a-change-applies).

| Value | Description |
|---|---|
| `ON_RESTART` | The default. Closing the settings after changing it offers to restart the app. |
| `IMMEDIATELY` | The code that reads it sees a change right away, so changing it never asks for a restart. |

### `ToggleSetting`

An on/off setting.

| Member | Description |
|---|---|
| `covers: List<ToggleSetting>` | The toggles it switches on while it is on. |
| `summaryOn: StringRef?`, `summaryOff: StringRef?` | The line under the title while it is on, or off. |
| `copy(default: Boolean = this.default): ToggleSetting` | The same setting, under the same key, with another default. A patch shows the copy when the default depends on its options or on the app version. |

### `TextSetting`

A text setting.

| Member | Description |
|---|---|
| `multiline: Boolean` | Whether it holds one entry per line. |
| `showValue: Boolean` | Whether the row shows the value. When false it shows the summary, and the editor shows the value. |

### `NumberSetting`

A number setting. Extensions read it as a [`NumberValue`](reference_runtime.md#numbervalue).

| Member | Description |
|---|---|
| `slider: Slider` | The slider it is picked on. |

### `RangeSetting`

A setting that holds a start and an end. Extensions read it as a [`RangeValue`](reference_runtime.md#rangevalue).

| Member | Description |
|---|---|
| `slider: Slider` | The slider whose two thumbs pick it. |

### `Slider`

`Slider(min: Double, max: Double, step: Double, unit: String = "", logarithmic: Boolean = false, scale: Double = 1.0, minLabel: StringRef? = null, maxLabel: StringRef? = null, dialog: Boolean = false)` is how a number or range setting is picked. It runs from `min` to `max` in steps of `step`.

The row shows the summary over the slider, with the labels of its ends, and the value under it. A value shows times `scale`, followed by `unit`. At an end with a `minLabel` or `maxLabel`, the label shows instead.

A `logarithmic` slider spreads its positions over orders of magnitude, for ranges such as view counts. A `dialog` slider shows only the summary in its row, and a tap opens the slider in a dialog that stores the value on Save.

A tap on a row with the slider in it opens the value as text, so the user can type an exact one. An entry off the slider is refused.

### `ColorSetting`

A color setting. Its row shows the summary beside a swatch of the color, and a tap opens a picker with hue, saturation and brightness sliders and the color code. Extensions read it as a [`ColorValue`](reference_runtime.md#colorvalue).

| Member | Description |
|---|---|
| `opacity: Boolean` | Whether the picker also sets the alpha. Without it the color is opaque. |
| `choice: ChoiceSetting?` | The choice picked with the color in the same dialog, if any. Its row shows the chosen title when the color has no summary. |

### `FolderSetting`

A folder setting. Its value is the folder's address, as text. Settings export leaves it out, because the permission to the folder belongs to this install.

### `FileSetting`

A file setting. Its value is the file's `content://` address, as text. Settings export leaves it out, because the permission to the file belongs to this install.

| Member | Description |
|---|---|
| `mimeTypes: List<String>` | The kinds of file the picker offers. |

### `ChoiceSetting`

| Member | Description |
|---|---|
| `choices: List<Choice>` | The values the user picks from. |
| `copy(default: String = this.default, choices: List<Choice> = this.choices): ChoiceSetting` | The same setting, under the same key, with other choices. A patch uses it to offer choices that depend on its options. |

### `Choice`

`Choice(value: String, title: DisplayText)`: the stored `value`, and the `title` users see.

### `ActionSetting`

A row that runs code when the user taps it. It stores nothing, so settings export and import skip it, and extensions have no value to read for it.

### `NoteSetting`

A row that only shows its summary, or the view a patch draws with `onView`. It stores nothing.

## Text

### `StringTable`

Translated strings, generated from the `strings.xml` files in a patch or settings module's `src/main/strings`. The generated object has one `StringRef` member per string without arguments, named in camel case: `feed_style_compact` becomes `ExampleStrings.feedStyleCompact`.

| Member | Description |
|---|---|
| `id: String` | The table's name in the app. |

### `StringRef`

One string of a table. The app shows a setting's text in its current language, and Reseam Manager shows a patch's name, description and option text in the user's language.

| Member | Description |
|---|---|
| `table: StringTable` | Its table. |
| `name: String` | Its name in `strings.xml`. |

### `Verbatim`

`Verbatim(text: String)`: text shown as it is in every language, such as a name the user typed into an option.

### `DisplayText`

Text a settings screen shows: a `StringRef` or a `Verbatim`.

## Laying out the screen

### `SettingsContribution`

Settings a patch gives a settings host: a `SettingsSection`, shown in rows, or `StoredSettings`, kept without rows. Its `settings: List<Setting<*>>` are the settings, in order.

### `SettingsSection`

A group of settings under a heading.

| Member | Description |
|---|---|
| `title: StringRef?` | The heading. Without one, the settings continue the section before them. |
| `settings: List<Setting<*>>` | The settings, in order. |
| `page: SettingsPage?` | The page it is on, or null for the main screen. |

### `StoredSettings`

Settings a patch's code keeps that no row shows, which `stored` makes.

### `section`

| Function | Description |
|---|---|
| `section(title: StringRef, vararg settings: Setting<*>): SettingsSection` | A section on the main screen. |
| `section(page: SettingsPage, title: StringRef, vararg settings: Setting<*>): SettingsSection` | A section on a page. Sections with the same page and title merge, so several patches can add to one section. |
| `section(page: SettingsPage, vararg settings: Setting<*>): SettingsSection` | Settings on a page under no heading. |
| `stored(vararg settings: Setting<*>): StoredSettings` | Settings a patch's code keeps that no row shows, such as values its own dialog changes. Settings export and import carry them, and extensions read the defaults patches give them. A patch returns them from `PatchBuilder.settings(host) { }`, and a host lists them in `layout`. |

When several patches show the same setting in one section, it keeps the place the first patch gave it and takes the definition the last one showed. A patch runs after the patches it depends on, so it can show a `copy` of their setting with the default or choices its own options call for.

### `SettingsPage`

A page of its own, shown as a row on its parent.

| Member | Description |
|---|---|
| `SettingsPage(id: String, title: StringRef, parent: SettingsPage? = null, order: Int = 0, summary: StringRef? = null, icon: Vector? = null)` | `parent` nests it in another page. Lower `order` comes first among its siblings. Page rows come before the settings, unless the host places the page after a setting with `place`. |
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
| `layout(vararg contributions: SettingsContribution)` | Where and in what order the settings patches show appear. A shown setting listed here takes the page, heading and place of its section in the layout, and the layout's sections come first on their page, in its order. A shown setting listed in `stored` loses its row and keeps its value. Listing a setting does not show it: one no patch that ran shows stays off the screen. Shown settings the layout leaves out stay in their patches' sections, after the listed ones. A setting may be listed on several pages, once on each, and not both shown and stored. |
| `place(page: SettingsPage, after: Setting<*>, heading: StringRef? = null)` | Shows `page`'s row on its parent right after `after`'s row, while that setting shows. Without a placement, page rows lead their parent. With a `heading`, the row starts a new group under it, and the section's later rows follow it there. |
| `strings(vararg tables: StringTable)` | String tables your extensions read. The tables your settings use ship anyway. |
| `brand(brand: Brand)` | How the settings screen and your branded components look. |
| `about(about: About)` | Links and buttons on the screen's about card. |
| `install(block: PatchRuntime.() -> Unit)` | Connects the screen to the app, typically a row in the app's own settings that calls `openSettings()`. It runs once every patch has registered its settings. |

### `SettingsHost`

What `settingsHost` returns: a [`ReseamPatch`](reference.md#reseampatch), with all its members, that you pass to `PatchBuilder.settings`.

| Member | Description |
|---|---|
| `appId: String` | The app's short name. |
| `register(patch: ReseamPatch, sections: List<SettingsContribution>)` | Adds a patch's settings. `PatchBuilder.settings` calls it for you. |
| `showIconsOf(keys: Set<String>)` | Shows only the icons of the pages and settings whose ids or keys are in `keys`, on this run's screen. Without a call every icon shows. Call it in `execute`. When several patches call it, the last one decides. |

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
| `CodeScope.addReactPackage(packages: ValueRef, addTo: ExtMethod): ValueRef` | Adds a React Native package your bundle ships, through `addTo`, a static method of the package's class that adds the package to the list it gets. The class is made to implement the app's package interface, so it names no React Native package type. |

### `onAction`

`onAction(action: ActionSetting, block: ActionScope.() -> Unit)` runs `block` in the app each time the user taps `action` on your settings screen. Call it in `execute` of the patch that shows the action, or of another patch in the same run.

```kotlin
import app.reseam.patch.settings.onAction

execute {
    onAction(AppSettings.exportLogin) {
        Login.copy(activity)
    }
}
```

Every action your settings show needs code. The settings host fails, and names the action, when one has none.

### `enabledWhile`

`enabledWhile(setting: Setting<*>, toggle: ToggleSetting)` makes `setting`'s row take changes only while `toggle` is on. When a run calls it more than once for a setting, the last call counts.

### `onSummary`

`onSummary(setting: Setting<*>, block: SummaryScope.() -> Unit)` computes the row's summary in the app, when the row shows and after any setting changes. `block` returns a `CharSequence` with `returnValue`. Returning null, or not returning, shows the declared summary.

### `onView`

`onView(setting: Setting<*>, block: ViewScope.() -> Unit)` draws a view under the row's title and summary, such as a preview of what the settings around it do. `block` returns an `android.view.View` made with `context`. The row redraws it after any setting changes.

To show the view alone, give it a `note` with neither a title nor a summary string.

### `onEnabled`

`onEnabled(setting: Setting<*>, block: EnabledScope.() -> Unit)` decides in the app whether the row takes changes. `block` returns a boolean with `returnValue`, `returnTrue` or `returnFalse`. Not returning leaves the row enabled. The app asks again after any setting changes. With `enabledWhile` too, the row needs both.

### `onValidate`

`onValidate(setting: TextSetting, block: ValidateScope.() -> Unit)` checks the text the user entered before it is stored. `block` returns an error message, a `CharSequence`, with `returnValue`. The old value stays, the message shows, and the editor opens again with the text. Returning null, or not returning, stores the text.

### `onChange`

`onChange(setting: Setting<*>, block: ChangeScope.() -> Unit)` runs `block` in the app after the user changes `setting` on the settings screen. Changes made by code, or by importing settings, don't run it.

### `ActionScope`

The code block of `onAction`, a [`CodeScope`](reference_code.md#codescope). The code runs in a static method, so it has no `thisObject`.

| Member | Description |
|---|---|
| `activity: ValueRef` | The `Activity` your settings are shown over. |

### `SummaryScope`

The code block of `onSummary`, a [`CodeScope`](reference_code.md#codescope) without `thisObject`.

| Member | Description |
|---|---|
| `value: ValueRef` | The setting's current value, as a `String`. |
| `context: ValueRef` | The settings screen's `Context`. |

### `ViewScope`

The code block of `onView`, a [`CodeScope`](reference_code.md#codescope) without `thisObject`.

| Member | Description |
|---|---|
| `context: ValueRef` | The settings screen's `Context`, to make the view with. |

### `EnabledScope`

The code block of `onEnabled`, a [`CodeScope`](reference_code.md#codescope) without `thisObject`.

| Member | Description |
|---|---|
| `context: ValueRef` | The settings screen's `Context`. |

### `ValidateScope`

The code block of `onValidate`, a [`CodeScope`](reference_code.md#codescope) without `thisObject`.

| Member | Description |
|---|---|
| `value: ValueRef` | The text the user entered, as a `String`. It is not stored yet. |

### `ChangeScope`

The code block of `onChange`, an [`ActionScope`](#actionscope), so `activity` is the screen's `Activity`.

| Member | Description |
|---|---|
| `previous: ValueRef` | The value before the change, as a `String`, so a confirmation can put it back. |

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

`Vector.svg(source: String): Vector` reads an SVG when your patches load. The `svg` element needs a `viewBox`, or a `width` and `height`, with a positive size. These elements draw: `path`, `circle`, `ellipse`, `rect`, `line`, `polyline`, and `polygon`, inside any number of `g`. Paint comes from `fill`, `stroke`, `stroke-width`, `stroke-linecap`, `stroke-linejoin`, and `fill-rule`, inherited from enclosing elements. `currentColor` takes the color the screen draws icons in. A `#rgb` or `#rrggbb` color stays fixed, and `none` paints nothing. Anything else, such as transforms, `style`, gradients, or text, fails the build.

```kotlin
private object Icons

val megaphone = Vector.svg(Icons::class.java.getResource("/icons/megaphone.svg")!!.readText())
```

`Vector.lucide(name: String): Vector` is the [Lucide](https://lucide.dev/icons) icon `name`, such as `"trash"`, drawn as Lucide draws it: a round stroke 2 wide in the screen's icon color. The patch API ships every icon of Lucide 1.66.0, so bundles carry no Lucide files. An unknown name, including Lucide's old aliases such as `"trash-2"`, fails the build.

### `About`

`About(links: List<Link> = emptyList(), actions: List<Link> = emptyList())`: where people reach you, on the settings screen's about card. `links` show as icon buttons under your name, and `actions` as buttons below them.

### `Link`

`Link(label: DisplayText, url: String, icon: Vector)`: one link. `url` must be an `https://` address. An icon button shows no label, but screen readers read it aloud.

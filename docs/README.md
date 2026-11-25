# Writing patches

These pages are published at [reseam.app/docs](https://reseam.app/docs/). Read the guide in order the first time.

1. [Start a bundle](1_start.md)
2. [Your first patch](2_first_patch.md)
3. [How patching works](3_how_patching_works.md)
4. [Declaring patches](4_patches.md)
5. [Finding code](5_finding_code.md)
6. [Pointing at instructions](6_points.md)
7. [Changing code](7_changing_code.md)
8. [Manifest, resources, and files](8_app_files.md)
9. [Testing and debugging](9_testing.md)
10. [Shipping your own code](10_extensions.md)
11. [Settings inside the app](11_settings.md)
12. [Reading obfuscated objects](12_bindings.md)
13. [Raw bytecode](13_bytecode.md)
14. [Hermes JavaScript](14_hermes.md)
15. [Native code](15_native.md)
16. [Bundle projects](16_bundles.md)
17. [Publishing](17_publishing.md)

The API reference lists every public type and member:

- [Patches](reference.md): declaring patches, options, and what a patch sees while it runs
- [Targets and points](reference_targets.md): finding methods, classes, fields, instructions, and values
- [Code blocks](reference_code.md): adding code, values, extension bindings, and app-wide changes
- [Manifest, resources, and files](reference_files.md)
- [Settings](reference_settings.md): settings, the settings screen, gates, and brands
- [Hermes](reference_hermes.md): JavaScript functions in React Native apps
- [Native libraries](reference_native.md): functions in a native ARM library
- [Raw bytecode](reference_dex.md): methods, classes, and instructions as data
- [App runtime](reference_runtime.md): what your extensions call inside the app
- [Gradle plugin](reference_gradle.md): plugins, settings, and build properties

Look things up in [troubleshooting](troubleshooting.md) and the [glossary](glossary.md).

Engine internals for contributors are in [internals/](internals/).

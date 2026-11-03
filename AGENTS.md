# Reseam engine

This repo is the engine that patches Android apps. The Rust crates read and write DEX files, APKs, Android resources and Hermes bytecode. The patcher runs Kotlin patches against an app through a JNI bridge, and the SDK wraps all of it for the apps that use the engine (the CLI, the browser build, Manager). Patches themselves are in the sibling repo `../patches`.

Each file should do one thing, and each rule should be written once. Please keep it like that when you change things.

## Layout

The crates, from the bottom up. Lower crates don't depend on higher ones, and crates use each other only through what they export at the crate root.

- `bytes`, `storage`, `journal`: reading binary fields, file-backed data and temp files, undo for patch runs.
- `dex`, `hermes`: the DEX and Hermes bytecode formats. Reading, the in-memory model, editing and writing each have their own modules.
- `apk`, `sign`: APK files (ZIP, binary XML, resources, the app's DEX files) and APK signing.
- `model`: types shared between the engine and the apps that use it.
- `patcher`: patch bundles, choosing and ordering patches, running them, and the Kotlin bridge (`bridge/`).
- `sdk`: what host apps call. It has `native`, `browser` and `compression` builds. The CLI only depends on `sdk`, `model` and `storage`.
- `patch-api`: the Kotlin API patch authors use. Public API is at the package root, internals are in `run/`, `search/`, `points/`, `emit/`, `binding/`, `dex/` and `settings/`.
- Tooling: `xtask` (code generation and packaging), `gradle-plugin`, `browser`, `app-runtime` and `sdk-kotlin`. `fixtures` builds test inputs.

## Docs worth reading

- `README.md` for the layout, building and releasing.
- `docs/` is the guide for patch authors. The `docs/reference*.md` pages list the whole patch API and app runtime, and have to match `patch-api` and `app-runtime`.
- Each crate's `README.md`, and `crates/patcher/ARCHITECTURE.md` for how a patch run works.
- `docs/internals/boltffi.md` explains how Rust and the Kotlin patch API talk to each other.
- `crates/cli/docs/` covers the CLI.
- `../notes/RELEASING.md` covers release order and versions.

## Working on the code

- Read the code around what you're changing first, and write in the same style.
- Check whether something already does what you need before you write it. If it does, use it or extend it.
- Fix the actual cause. Don't add a special case for one input, a flag, a retry or a temporary workaround. Someone has to clean those up later.
- Update everything your change affects in the same change: Rust, Kotlin and TypeScript callers, `../patches`, generated bindings, docs (see [Docs](#docs)) and tests. Don't leave old and new versions side by side or add compatibility shims.
- Think about where the code belongs before writing it. If a file would end up doing two things, split it first. If you'd be writing the same logic a second time, pull it out and share it.
- Remove anything your change leaves unused.

## Things to avoid

- Files that mix jobs, like a model together with its parser and writer.
- Writing the same logic in two places: a reader and writer that each encode the format, the same check in Rust and Kotlin, a copied lookup loop, or a hand-written copy of a type that can be generated.
- Functions or types that just pass calls through, and re-exporting another crate.
- Code nobody uses: exports with no callers, public functions that only tests call, fields that are never set.
- Strings where a type belongs: ids built by joining strings, string keys, `bool` parameters that switch behavior, and error messages with the details baked into text.
- Hiding errors by returning a default or `None`, and panicking on bad input.
- Traits, generics, builders or extra layers that only have one user.
- Modules that depend on each other in a circle, or a lower layer reaching into a higher one.
- Comments that repeat what the code says, and section banner comments.
- Tests that just repeat the implementation, and test data built byte by byte.

## Types, errors and file formats

- Use proper types for data: newtypes for ids, enums for kinds and modes. Turn values into text only at the edges (CLI arguments, JSON, XML).
- Bundle manifests, the release index (`patches.json`), the SDK and browser types, and the patch API are read by other code, including older releases out in the wild. Keep them stable. If one really has to change, update everything that reads it and bump the engine version as described in `../notes/RELEASING.md`.
- Each library crate has a single `thiserror` error type that says what went wrong and where. Pass errors up with `?`. Only use `expect` for things the code itself guarantees, and say what that is in the message.

## Performance

- Keep DEX and APK data on disk and read the ranges you need. Avoid loading whole files or copying big buffers.
- If you change decoding, patching or writing, run `reseam perf` before and after and compare time and peak memory.

## Kotlin

- Keep the public `patch-api` small and make everything else `internal`. Generated bindings shouldn't show up in public signatures.
- Before you remove or change public API, check how `../patches` uses it.

## Comments and tests

- Only comment to explain something the code can't show, like a platform quirk or a format rule. Don't describe what the code does. The patch API needs doc comments, and so does any public item whose behavior isn't clear from its signature.
- Write tests for behavior users rely on. When you test the same thing with several inputs, use one table-driven test. Use `reseam-fixtures` and the shared test helpers for inputs, and test through the public API.

## Docs

The docs are published at reseam.app/docs, and patch authors rely on them. Keep them true to the code in the same change.

**When to update them.** Any change that adds, removes, renames or changes the behavior of something authors or CLI users see: a patch API type or member, an app runtime member, a Gradle plugin setting, a CLI command or flag, an error message the docs quote, or a default. Update every page that mentions it:

- `docs/reference*.md` for the patch API, the app runtime (`reference_runtime.md`) and the Gradle plugin (`reference_gradle.md`).
- The guide page in `docs/` that explains the feature, plus `troubleshooting.md` and `glossary.md` when they cover it.
- `crates/cli/docs/` for the CLI, checked against `reseam <command> --help`.
- The crate `README.md` and `docs/internals/` when you move or rename files they name.

**Check claims against the code.** Read the implementation before you describe behavior, and don't document what you can't confirm. A Kotlin code sample must compile against the current API, with its imports: hooks such as `before` and `point` are extension functions and need one.

**Reference structure.** Every public type gets its own ``### `Type` `` heading, or a row naming it in a table of related records. Under it, one sentence on what the type is for, then a `| Member | Description |` table: the full signature with parameter types, defaults and the return type, and one or two sentences on what it does and when it fails. Extension functions go in a function table on the page of the type they extend. Generated types nobody can reach from public API stay out.

**Guide structure.** Each page follows the same order: what the page is for in one or two sentences, the smallest working example, what each part does, variations, a "Things to know" list of pitfalls, then `Next: [Page](file.md).` Keep the `description:` frontmatter accurate. Pages are numbered for order (`9_testing.md`); the URL is the name without the number, so renumbering is safe but renaming isn't. Link to other pages by file name, and to headings by their anchor. Add a new page to `docs/README.md`.

**Tone.** Write the way you'd explain it to another developer: plain words, full sentences, and "you" for the reader.

- Say why before the rule: "Pin your keys before the first release. If you rename the property, the key changes and users lose the value."
- One idea per paragraph, and short paragraphs. Split a paragraph that packs several facts.
- Use the terms from `docs/glossary.md`, and add a term there when a page needs a new one.
- Examples use the imaginary `com.example.app` and realistic names, 4-space indented Kotlin in the style of `../patches`.
- Use `> [!WARNING]` only for mistakes that cost users data or silently break a patch.
- No em-dashes, no "not X, but Y", no sentences that are fragments joined by colons and semicolons, no filler ("simply", "just", "easily", "powerful"), and no explaining what a developer already knows.

**Before you call it done.** Check that every page you touched renders and that its links resolve: run `bun run docs:local` in `../website` and open the pages. For a new or changed sample, compile it in a copy of `../patches-template` built against your engine with `RESEAM_WORKSPACE`.

## Maintainability checks with Entanglement

Run the installed `entanglement` CLI on any non-trivial change. The entanglement skill has the full workflow.

1. Before editing, save a baseline somewhere outside the repo: `entanglement repo . --metrics nloc,cc,cogc,mi,structure --format json > <scratch>/baseline.json`.
2. After each round of edits, run it again and compare against the baseline with the skill's `compare.py --structure`.
3. Every file you add or change needs a maintainability index of at least 10, and ideally 20 or more. If a file drops below that, split it by what it does. Don't squash code onto fewer lines or split files at random just to raise the number. Generated files don't count.
4. Don't add new dependency cycles. If line count or complexity grows, make sure the growth is needed.

## Before you call it done

- `just check` runs everything. While working, run at least `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace --no-fail-fast`.
- Don't edit generated code by hand. Run `cargo xtask regen <target>` after changing exported JNI functions or model types, and `./gradlew :reseam-patch-sdk:updateKotlinAbi` after an intended patch API change.
- For changes to DEX, APK or emitted code, patch a few real apps before and after, compare the output, and run ART verification on the result.
- Leave `CARGO_BUILD_JOBS` as the environment sets it.
- Run the checks and read their output before saying something works.

## Commits

One line, `type(scope): subject`, with nothing below it.

- Say what changed in plain words, in 50 characters or fewer and never past 72. Leave the reason out: no "so ...", "instead of ..." or "by ..." tails.
- One change per commit. A subject that lists several changes with "and" is several commits.
- Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `style`, `chore`, `revert`. A breaking change to the patch API, bundle format or SDK types gets `!` after the scope.
- The scope is the crate or package: `apk`, `dex`, `hermes`, `sign`, `model`, `patcher`, `sdk`, `cli`, `patch-api`, `app-runtime`, `gradle-plugin`, `browser`, `xtask`. Leave it out when a change spans the engine.
- Releases are `chore: release vX.Y.Z`, which `cargo xtask release` writes.

```text
fix(apk): don't crash when saving after a DEX is deleted
perf(patcher): load only the patches a run executes
feat(patch-api)!: add declarative targets
```

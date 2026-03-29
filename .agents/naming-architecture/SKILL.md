---
name: naming-architecture
description: Family-first detection, per-language naming definitions, canonical save-extension mapping, and save/rename naming flows.
---

# Naming Architecture

Use this skill when changing language detection, filename suggestion, canonical save extensions, or the save/rename flows that bridge Rust naming with the Svelte UI.

## Primary Files

- `src-tauri/src/detection/mod.rs`
- `src-tauri/src/detection/features.rs`
- `src-tauri/src/detection/family.rs`
- `src-tauri/src/detection/scoring.rs`
- `src-tauri/src/detection/disambiguation.rs`
- `src-tauri/src/detection/structural.rs`
- `src-tauri/src/detection/languages/mod.rs`
- `src-tauri/src/detection/languages/*.rs`
- `src-tauri/src/naming/mod.rs`
- `src-tauri/src/naming/model.rs`
- `src-tauri/src/naming/prose.rs`
- `src-tauri/src/naming/shared.rs`
- `src-tauri/src/naming/languages/mod.rs`
- `src-tauri/src/naming/languages/*.rs`
- `src-tauri/src/commands/naming.rs`
- `src/lib/components/RenameFileDialog.svelte`
- `src/lib/editor/components/EditorWrapper.svelte`
- `src/lib/editor/config/languageExtensions.ts`
- `src/lib/editor/config/languageIconMap.ts`
- `src/lib/state/librarySidebar.svelte.ts`

## Current Architecture at a Glance

Detection and naming are intentionally separate systems that meet at a shared language ID.

- **Detection** answers: "what language does this content most likely belong to?"
- **Naming** answers: "what stem and canonical extension should we suggest/save with?"

Rust is the source of truth for both:

- content-based language detection
- content-based filename suggestion
- canonical save-extension mapping

The frontend keeps only thin synchronous maps for immediate UI behavior:

- `languageExtensions.ts` for CodeMirror language support
- `languageIconMap.ts` for sidebar/status-bar labels and icons

Do not move content-based detection or naming logic back into Svelte.

## Detection Architecture

## Family-first pipeline

`src-tauri/src/detection/mod.rs::detect_language()` now delegates to the v2 family-first pipeline.

Ordered phases:

1. filename / extension
2. shebang
3. strong structural detection
4. content family classification
5. soft structural detection (family-gated)
6. family-gated candidate scoring
7. superset/rival disambiguation
8. confidence gate (abstain if unsure)

The key architectural change is that content is classified into broad families before language-specific scoring runs:

- `Prose`
- `Code`
- `StructuredData`
- `Markup`
- `ShellScript`
- `Config`

That family gate dramatically reduces cross-family false positives. Prose should not compete with Scala, SQL, YAML, CMD, or PowerShell unless the content first looks like code/data/shell.

## Phase ownership

### Deterministic phases

- extension / filename: `extension.rs`
- shebang: `shebang.rs`
- strong structural detectors: `structural.rs` plus language-local structural functions

These still short-circuit early for highly reliable cases like JSON, HTML, XML, Dockerfile, etc.

### Family classification

- `features.rs` extracts language-agnostic signals such as:
  - greeting / closing presence
  - contractions and stopword-heavy prose signals
  - import/function/operator density
  - key-value density and indentation depth
  - tag density
  - pipes / redirects / env expansion counts
- `family.rs` scores those signals and picks the top family or top-two families when ambiguous

This stage is deterministic and rule-based. There is no ML model here.

### Family-gated language scoring

`src-tauri/src/detection/scoring.rs` only scores languages whose `content_families` intersect the chosen family set.

Each language carries v2 metadata in `LanguageDefinition`:

- `content_families`
- `anchors` — strong, near-exclusive signals
- `hints` — weaker supporting signals
- `keywords` — language-specific keyword tokens for fingerprinting
- `builtins` — language-specific builtin tokens for fingerprinting
- `disqualifiers` — hard rule-outs

The scoring function awards anchor + hint + keyword/builtin bonus (gated on anchor evidence). Minimum threshold (`ANCHOR_THRESHOLD = 4`) required for candidacy.

### Rival disambiguation

`disambiguation.rs` resolves close competitors such as:

- JavaScript vs TypeScript
- Angular vs TypeScript/JavaScript
- Java vs Kotlin vs Scala
- C vs C++
- Python vs Ruby / Perl

It uses superset pair relationships and score-gap analysis to break ties between ambiguous candidates.

## First-class prose languages

`email` and `prompt` are now real languages in detection, not just prose side-signals.

Detection files:

- `src-tauri/src/detection/languages/email.rs`
- `src-tauri/src/detection/languages/prompt.rs`

Both live in `ContentFamily::Prose`, which means:

- they do not compete with code families unless the family classifier is already wrong
- email vs prompt ambiguity is resolved with prose-specific anchors and disqualifiers

Examples:

- email anchors: RFC headers, greetings, closings, reply markers
- prompt anchors: role framing (`You are`, `Act as`), instruction verbs, output-format directives

## Adding a new detection language

1. Create `src-tauri/src/detection/languages/<lang>.rs`
2. Export `definition() -> LanguageDefinition`
3. Register the module and `definition()` in `src-tauri/src/detection/languages/mod.rs`
4. Populate:
   - deterministic fields: `extensions`, `filenames`, `shebangs`
   - family-gated fields: `content_families`, `anchors`, `hints`, `keywords`, `builtins`, `disqualifiers`
5. Add tests for:
   - positive detection
   - nearby-rival disambiguation
   - prose / mixed-content rejection where relevant

The family-first pipeline is the sole detection architecture. There is no legacy fallback.

## Naming Architecture

## Per-language naming registry

Naming no longer routes through the old `language_profile()/ExtractorGroup` map.

Current architecture:

- each language lives in `src-tauri/src/naming/languages/*.rs`
- each file exports `definition() -> NamingDefinition`
- `src-tauri/src/naming/languages/mod.rs` compiles them into `NAMING_MAP`

`NamingDefinition` is intentionally small:

- `name`
- `extension`
- `extract`

That means adding naming support for a language is usually:

1. create one file
2. register it in `all_definitions()`

The canonical save extension now lives directly on `NamingDefinition.extension`.

## Public naming API

`src-tauri/src/naming/mod.rs` is the stable surface:

- `suggest_stem_auto(content, language_hint, filename)`
- `suggest_stem(content, language_hint)`
- `language_to_extension(language_hint)`
- `fallback_stem()`
- `slugify(raw)`

### `suggest_stem_auto`

When `language_hint` is empty or `"auto"`:

- it runs `crate::detection::detect_language(content, filename)`
- falls back to `"text"` if detection abstains
- returns both the suggested stem and the effective language

That returned language must be propagated by callers rather than recomputed separately.

### `suggest_stem`

Flow:

1. bound content
2. look up `NamingDefinition`
3. extract raw stem
4. finalize / slugify / suffix-tag

Special cases:

- if `def.name == "text"`, route through `prose::extract_prose_tagged()`
- if `def.name == "email"` or `"prompt"`, force `StemKind::Email` / `StemKind::Prompt`

That explicit `StemKind` routing is important: once detection returns `"email"` or `"prompt"`, the generated filename must keep the `-email` or `-prompt` suffix even though those languages now have dedicated naming definitions.

## Prose naming

`src-tauri/src/naming/prose.rs` remains the prose fallback cascade:

1. email extraction
2. prompt extraction
3. YAKE fallback

The new naming language files:

- `src-tauri/src/naming/languages/email.rs`
- `src-tauri/src/naming/languages/prompt.rs`

both delegate to that prose extractor, but they preserve distinct language IDs and therefore distinct suffix behavior.

## Detection extension vs save extension

Keep this distinction clear:

- detection may accept many aliases for one language
- naming uses one canonical save extension from `NamingDefinition.extension`

Examples:

- detect many Perl-related extensions, save as `.pl`
- detect multiple Scala-ish filenames, save as `.scala`
- unknown languages fall back to the `text` naming definition and save as `.txt`

Do not derive canonical save extensions from frontend icon maps or old file extensions.

## Command / IPC Flow

## `save_untitled_slate`

`src-tauri/src/commands/naming.rs::save_untitled_slate`:

1. resolves notes root
2. calls `suggest_stem_auto`
3. maps the effective language through `language_to_extension`
4. sanitizes and de-duplicates the filename
5. writes the file
6. records a save event in storage
7. emits `RECENT_FILES_UPDATED_EVENT`
8. returns `SaveResult { path, detectedLanguage }`

This command is the source of truth for first-save naming. The frontend should not assemble untitled save paths by itself.

## `suggest_slate_name`

Use when the frontend already has the live content in memory and wants:

- `filename`
- `detectedLanguage`

This is the "content in, filename out" helper for Save As and live rename suggestions.

## `suggest_name_for_file`

Use when the frontend only has a path.

Important behavior:

- reads only a bounded sample from disk
- uses `"auto"` detection internally
- intentionally does **not** trust the existing file extension as a naming hint

That prevents misnamed files from being locked into the wrong canonical extension.

## Frontend Flows

## `EditorWrapper.svelte`

Relevant contracts:

- content-based detection remains Rust-side via `invoke("detect_language")`
- saved-file language pinning still uses path/filename sync detection (`detectByFilename`) so the status bar reflects the actual saved extension
- untitled save calls `save_untitled_slate`
- name generation for live content calls `suggest_slate_name`
- opening a file uses `setPendingSidebarOpenFile(...)` and `OPEN_FILE_PATH_EVENT`

Important sidebar interaction:

- after a successful `read_file_content`, the **backend** records the open event and emits `RECENT_FILES_UPDATED_EVENT`
- `EditorWrapper` should not manually emit recent-files refresh events anymore

## `RenameFileDialog.svelte`

The Generate Name button mirrors untitled naming as closely as possible:

- if the file is the current editor file, use live editor content + `suggest_slate_name(..., "auto")`
- otherwise call `suggest_name_for_file(path)`

After rename succeeds:

1. call `librarySidebarState.requestQuietDataRefresh?.()` so the sidebar picks up the new filename/path without visible jitter
2. if the renamed file is the active editor file, set `librarySidebarState.lastRenamedPath = { from, to }`
3. then update `editorState.currentFilePath = newPath`

That ordering is important. The sidebar uses `lastRenamedPath` to update its suppression tracking instead of misclassifying the rename as an external navigation.

## Frontend language support

If a detected/named language should also have first-class editor UI support, update:

- `src/lib/editor/config/languageExtensions.ts`
- `src/lib/editor/config/languageIconMap.ts`

Current notable behavior:

- `email` and `prompt` intentionally render as plain text in CodeMirror
- `cmd` uses the shell highlighter as the closest available CM mode
- `languageIconMap.ts` is the user-facing label/icon source for sidebar cards and the status bar

## Adding a New Language End-to-End

1. Add detection definition in `src-tauri/src/detection/languages/`
2. Register it in `detection/languages/mod.rs`
3. Populate v2 family fields
4. Add naming definition in `src-tauri/src/naming/languages/`
5. Register it in `naming/languages/mod.rs`
6. Add CodeMirror/icon support if user-visible
7. Add tests near the changed modules

## Safe Change Checklist

- Keep Rust as the source of truth for content-based detection and naming
- Preserve the distinction between detection aliases and canonical save extension
- When adding languages, populate `content_families`, `anchors`, `hints`, `keywords`, `builtins`, and `disqualifiers`
- Keep `email` / `prompt` suffix behavior intact
- Do not trust the current filename extension inside `suggest_name_for_file`
- Do not reintroduce frontend-only recent-files refresh emits for read/save flows
- Re-run:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `pnpm run check`

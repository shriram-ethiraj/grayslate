---
name: naming-architecture
description: Content-based filename suggestion, canonical save-extension mapping, per-language detection config, and FE save/rename naming flows.
---

# Naming Architecture

Use this skill when changing filename suggestions, adding a new language, or modifying how save and rename flows choose a filename or extension.

## Primary Files

- `src-tauri/src/naming/mod.rs`
- `src-tauri/src/naming/model.rs`
- `src-tauri/src/naming/shared.rs`
- `src-tauri/src/naming/structured.rs`
- `src-tauri/src/naming/markup.rs`
- `src-tauri/src/naming/code.rs`
- `src-tauri/src/naming/prose.rs`
- `src-tauri/src/naming/sql.rs`
- `src-tauri/src/commands/naming.rs`
- `src-tauri/src/detection/mod.rs`
- `src-tauri/src/detection/extension.rs`
- `src-tauri/src/detection/shebang.rs`
- `src-tauri/src/detection/structural.rs`
- `src-tauri/src/detection/heuristic.rs`
- `src-tauri/src/detection/treesitter.rs`
- `src-tauri/src/detection/languages/mod.rs`
- `src-tauri/src/detection/languages/*.rs`
- `src/lib/editor/components/EditorWrapper.svelte`
- `src/lib/components/RenameFileDialog.svelte`
- `src/lib/editor/core/detectByExtension.ts`
- `src/lib/editor/config/languageExtensions.ts`
- `src/lib/editor/config/languageIconMap.ts`

## Current Architecture at a Glance

Naming and detection are tightly coupled, but they solve different problems:

- **Detection** answers: "what language is this content?"
- **Naming** answers: "what stem should we suggest, and what canonical extension should we save with?"

Those are intentionally separate systems.

### Important distinction: detection extensions vs save extension

Detection can recognize many extensions for a language, but naming picks one canonical extension for saves and suggestions.

Examples:

- Scala detection accepts `.scala`, `.sc`, and `.sbt`, but naming saves as `.scala`
- Perl detection accepts `.pl`, `.pm`, `.perl`, `.pod`, and `.t`, but naming saves as `.pl`

If a language is detectable but missing from `language_profile()` in `src-tauri/src/naming/mod.rs`, saves and generated names fall back to `.txt`.

## Detection Architecture

### Per-language config lives in `src-tauri/src/detection/languages/*.rs`

Each language file owns its own detection fingerprint via `LanguageDefinition`.

Current fields include:

- `name`
- `extensions`
- `filenames`
- `filename_patterns`
- `shebangs`
- `structural_priority`
- `structural_detect`
- `patterns`
- `anti_patterns`
- `uses_hash_comments`
- `keywords`
- `builtins`
- `illegal`
- `extends`

This means extension, shebang, structural, and heuristic config all live with the language itself rather than being scattered across separate hardcoded maps.

### Registry compilation

`src-tauri/src/detection/languages/mod.rs` compiles the per-language definitions into shared registries used by the pipeline:

- `SUPPORTED_LANGUAGES`
- `EXTENSION_MAP`
- `FILENAME_MAP`
- `FILENAME_PATTERNS`
- `SHEBANG_MAP`
- `STRUCTURAL_DETECTORS`
- `COMPILED`

`extension.rs`, `shebang.rs`, `structural.rs`, and `heuristic.rs` now consume those registries instead of maintaining their own language-specific config.

### Detection pipeline

`src-tauri/src/detection/mod.rs::detect_language(content, filename)` runs the 4-phase cascade:

1. extension / filename
2. shebang
3. structural
4. heuristic scoring + tree-sitter validation

The FE should continue treating Rust as the source of truth for content-based detection.

### Current reality for adding a language

Per-language config is now self-contained in the language file, but the registry is still manually wired.

Today, a new backend language usually means:

1. create `src-tauri/src/detection/languages/<lang>.rs`
2. add `mod <lang>;` in `src-tauri/src/detection/languages/mod.rs`
3. add `<lang>::definition()` to `all_definitions()`

Do not assume file-only auto-discovery exists yet.

### Structural and heuristic guidance

- Data/markup formats like JSON, CSV, XML, YAML, TOML, Markdown, and plain text can stay primarily structural. That is expected.
- Code and template languages that lack strong structural signals should carry their own regex patterns, keywords, and builtins in their language file.
- Highlight.js is a reasonable source for language keywords and builtins when expanding heuristic coverage.

### `extends` is risky

Be conservative with `extends`.

Inheritance can make the child language outscore the parent on pure parent content. This already happens easily for near-superset pairs such as:

- TypeScript over JavaScript
- C++ over C
- Kotlin over Java
- Angular over TypeScript
- Objective-C++ over C++
- SCSS/Sass over CSS

Default to `extends: None` unless tests prove inheritance is safe for that language pair.

### Heuristic threshold reminder

`src-tauri/src/detection/heuristic.rs` only applies keyword/builtin bonus when there are at least 3 unique hits.

Avoid generic single-keyword or single-pattern signals that can hit the global threshold by accident. A pattern like `print something` is too weak by itself; prefer language-specific syntax such as sigils, delimiters, or distinctive constructs.

## Naming Architecture

### Public API

`src-tauri/src/naming/mod.rs` is the stable naming surface:

- `suggest_stem(content, language_hint)`
- `suggest_stem_auto(content, language_hint, filename)`
- `language_to_extension(language_hint)`
- `slugify(raw)`
- `fallback_stem()`

### `suggest_stem_auto`

`suggest_stem_auto(content, language_hint, filename)` is the bridge between detection and naming.

- if `language_hint` is empty or `"auto"`, it runs `detect_language(content, filename)`
- it returns both the suggested stem and the effective language
- callers should propagate that effective language instead of recomputing it separately

### `language_profile()` is the canonical save-extension map

`src-tauri/src/naming/mod.rs::language_profile()` maps a language ID to:

1. the canonical save extension
2. the extractor group used to derive the filename stem

This mapping is separate from detection and must be updated whenever a newly supported language should save with a non-`.txt` extension.

### Extractor routing

`model.rs` contains the routing model:

- `LanguageNamingProfile`
- `ExtractorGroup`
- `StructuredNamingKind`
- `MarkupNamingKind`
- `CodeStyle`

Keep language-to-extractor dispatch centralized in `language_profile()` rather than spreading extension logic across command handlers.

## Extractor Responsibilities

### `shared.rs`

- bounded input sampling
- slug sanitization
- timestamp fallback naming

Keep this file generic and language-agnostic.

### `structured.rs`

Structured formats:

- CSV: semantic header extraction with noise filtering
- JSON: known file shapes plus semantic key/value extraction
- YAML: key-based extraction
- TOML: AST-first extraction with pattern-aware fallbacks

### `markup.rs`

Markup/document formats:

- XML/HTML-like markup
- Markdown
- Svelte/Vue/Angular naming when routed through markup-style extraction

### `code.rs`

Programming language extraction:

- tree-sitter-backed extraction for grammars already in the project
- regex fallback for styles without tree-sitter coverage in naming

Current regex-fallback styles include:

- CSharp
- Swift
- Ruby
- PHP
- Dart
- Shell

If tree-sitter parsing is absent or not yet wired for naming, prefer reusing an existing fallback style before inventing a new extractor.

### `prose.rs`

Fallback extractor cascade:

1. email detection
2. prompt detection
3. YAKE keyword extraction

This is also the current fallback for some detected languages whose naming extractor has not been specialized yet.

### `sql.rs`

All SQL-specific filename inference belongs here.

Do not spread SQL naming heuristics into command handlers or unrelated extractors.

## Command and IPC Flow

### `save_untitled_slate`

`src-tauri/src/commands/naming.rs::save_untitled_slate` handles first save for untitled documents:

1. resolve notes root
2. call `suggest_stem_auto`
3. call `language_to_extension`
4. sanitize and de-duplicate the target filename
5. write the file
6. record the save event
7. return `SaveResult { path, detectedLanguage }`

### `suggest_slate_name`

`suggest_slate_name(content, language_hint)` is the content-in / filename-out helper used when the FE already has the document text.

It returns:

- `filename`
- `detectedLanguage`

Use this when the frontend already has content in memory and wants the generated name to reflect that exact content.

### `suggest_name_for_file`

`suggest_name_for_file(path)` is the disk-backed helper for existing files when the FE only has a path.

Important behavior:

- reads a bounded sample from disk
- uses `"auto"` content detection
- intentionally does **not** use the file extension as the language hint
- returns a full filename string (`stem.extension`)

This prevents misnamed files from getting locked into the wrong extension during name generation.

Do not feed raw file extensions like `"pl"` or `"py"` into `language_profile()` and expect canonical naming behavior. `language_profile()` wants language IDs like `"perl"` or `"python"`.

## Frontend Flow

### `EditorWrapper.svelte`

Key behavior:

- content-based detection stays in Rust via `invoke("detect_language")`
- untitled save calls `save_untitled_slate`
- untitled Save As calls `suggest_slate_name`
- the FE still keeps a thin `detectByExtension.ts` map for sync, extension-only UI decisions like icons

### `RenameFileDialog.svelte`

The "Generate name" button should mirror untitled naming behavior as closely as possible.

Current behavior:

- if the file being renamed is the **currently open file**, use live editor content and call `suggest_slate_name` with `languageHint: "auto"`
- this includes unsaved in-editor changes in the suggestion
- if the file is **not** the currently open file, fall back to `suggest_name_for_file(path)` so the backend reads the file from disk

In both cases, the generated name should include the extension derived from content, not just the old filename.

## Adding a New Language End-to-End

### Backend detection

1. create a new file under `src-tauri/src/detection/languages/`
2. define extensions, filenames, shebangs, structural detector, and heuristics in that file
3. register the module and `definition()` in `src-tauri/src/detection/languages/mod.rs`

### Backend naming

4. add a `language_profile()` entry in `src-tauri/src/naming/mod.rs`
5. choose the canonical save extension
6. route the language to the best existing extractor group, or add new extractor support if truly needed

### Frontend language support

If the language should render with syntax highlighting and app-specific UI support, also update:

7. `src/lib/editor/config/languageExtensions.ts`
8. `src/lib/editor/config/languageIconMap.ts`

### Tests

Add or update tests close to the changed module:

- detection extension / filename cases
- shebang cases
- structural detection
- heuristic scoring
- naming extension mapping
- command-level naming behavior where appropriate

## Safe Change Checklist

- Keep per-language detection config inside the language file
- Keep canonical save-extension routing in `language_profile()`
- Preserve the distinction between detection extensions and save extension
- Prefer content-based `"auto"` flows when suggesting names from live content
- Do not assume the old file extension is trustworthy for rename suggestions
- Be very cautious with `extends`
- Add tests for new language routing and extension behavior
- Re-run:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `pnpm run check`
  - `pnpm run build`

---
name: naming-architecture
description: Untitled slate naming architecture, language-profile dispatch, SQL naming, tree-sitter code extraction, email/prompt detection, and structured data semantic naming.
---

# Naming Architecture

Use this skill when changing untitled-slate filenames, adding support for a new language or extension, or modifying naming heuristics for any content type.

## Primary Files

- `src-tauri/src/naming/mod.rs`
- `src-tauri/src/naming/model.rs`
- `src-tauri/src/naming/shared.rs`
- `src-tauri/src/naming/structured.rs`
- `src-tauri/src/naming/markup.rs`
- `src-tauri/src/naming/code.rs`
- `src-tauri/src/naming/prose.rs`
- `src-tauri/src/naming/sql.rs`
- `src-tauri/src/detection/mod.rs` (language detection pipeline)
- `src-tauri/src/detection/extension.rs`
- `src-tauri/src/detection/shebang.rs`
- `src-tauri/src/detection/structural.rs`
- `src-tauri/src/detection/heuristic.rs`
- `src-tauri/src/detection/treesitter.rs`
- `src-tauri/src/commands/naming.rs`
- `src-tauri/src/commands/detection.rs`
- `src/lib/editor/components/EditorWrapper.svelte`
- `src/lib/editor/core/detectByExtension.ts` (thin FE extension map for sync sidebar use)

## Rust Module Shape

Rust’s module equivalent is `mod`.

In this area, the naming logic is intentionally organized as a directory module:

- `src-tauri/src/naming/mod.rs` is the public entrypoint.
- sibling files under `src-tauri/src/naming/` hold focused implementation details.

Keep the Tauri command layer importing the stable public functions from `crate::naming` instead of reaching into submodules directly.

## Public API Contract

`src-tauri/src/naming/mod.rs` is the stable surface consumed by commands:

- `suggest_stem(content, language_hint)`
- `suggest_stem_auto(content, language_hint)` — auto-detects language when hint is "auto" or empty
- `language_to_extension(language_hint)`
- `slugify(raw)`
- `fallback_stem()`

`src-tauri/src/detection/mod.rs` is the language detection surface:

- `detect_language(content, filename)` — 4-phase pipeline: extension → shebang → structural → heuristic+tree-sitter

If you need new internal helpers, add them in submodules and keep this public surface small unless the command layer truly needs more.

## Dispatch Model

`model.rs` contains the scalable routing model:

- `LanguageNamingProfile`
- `ExtractorGroup`
- `StructuredNamingKind`
- `MarkupNamingKind`
- `CodeStyle`

`mod.rs` owns the `language_profile()` match that maps a language hint to:

1. the canonical file extension
2. the extractor group to run

### Rule for adding a new language

Prefer extending `language_profile()` and reusing an existing extractor group before creating a brand-new extractor module.

Examples:

- new JSON-adjacent structured format → likely `structured.rs`
- new XML/HTML-like template language → likely `markup.rs`
- new programming language with a tree-sitter grammar → add grammar crate + `CodeStyle` variant in `code.rs`
- new programming language without a tree-sitter grammar → add regex patterns in the fallback path of `code.rs`
- SQL dialect-specific naming work → `sql.rs`

## Extractor Responsibilities

### `shared.rs`

- bounded input sampling (`MAX_CONTENT_BYTES`)
- slug sanitization
- timestamp fallback naming

This file should stay generic and free of language-specific heuristics.

### `structured.rs`

For delimiter/key-oriented formats:

- **CSV**: noise column filtering (skips IDs, timestamps, coordinates, generic names), then takes up to MAX_TOKENS semantic headers
- **JSON**: known-pattern detection (package.json, OpenAPI, tsconfig, GeoJSON, JSON Schema) → value extraction for `name`/`title`/`error` keys → noise key filtering → first N keys; falls back to regex for partial JSON
- **YAML**: regex-based `key:` extraction
- **TOML**: taplo AST parsing → known-pattern detection (Cargo.toml, pyproject.toml, Poetry, Hugo) → name-value extraction from `[section]` tables → noise section filtering; falls back to regex when taplo fails

### `markup.rs`

For tree/document formats:

- XML/HTML-like markup
- Markdown

### `code.rs`

For programming languages. Uses **tree-sitter** AST parsing for languages with grammar crates, with **regex fallback** for languages without grammars.

#### tree-sitter covered languages

- Python, JavaScript, TypeScript, Rust, Java, Go, C, C++

#### Code signal priority

- 10: Module/package/namespace declaration
- 9: Public/exported class, struct, trait, interface
- 8: Other public type declarations, exported functions
- 7: Public functions
- 5-6: Private/unexported declarations

#### Noise names filtered

`main`, `init`, `setup`, `run`, `start`, `new`, `default`, `handle`, `index`, `app`, `mod`, `test`, `self`, `this`, `cls`

#### Regex fallback languages

CSharp, Swift, Ruby, PHP, Dart, Shell — these don't have tree-sitter grammar crates in the project.

To add tree-sitter support for a new language: add the grammar crate to Cargo.toml (must be compatible with `tree-sitter = "0.24"`), add a mapping in `try_tree_sitter()`, and add a `collect_<lang>()` function.

### `prose.rs`

Cascade extractor for unknown/prose-like content:

1. **Email detection** → extract Subject line (strips Re:/Fwd:/[bracket] prefixes, preserves ticket IDs)
2. **Prompt detection** → extract role ("You are a ...") or task ("Write/Create/Generate a ...")
3. **YAKE keyword extraction** → statistical keyphrase fallback

Email and prompt content stays with `.txt` extension — the naming captures what the content is about.

### `sql.rs`

All SQL-specific filename inference belongs here.

Do not spread SQL heuristics across `mod.rs`, `commands/naming.rs`, or unrelated extractor files.

## SQL Naming Rules

`sql.rs` uses `sqlparser-rs` and ranks naming signals by semantic importance.

Current high-priority signals include:

- first CTE name
- last/output CTE name
- `CREATE TABLE` / `CREATE VIEW` target
- primary `FROM` table
- `INSERT` / `UPDATE` target
- `GROUP BY` columns
- `ORDER BY` columns
- `WHERE` filter columns
- joined tables

### Important SQL behavior

- Generic CTE aliases like `step1`, `cte2`, or `tmp3` should not dominate the filename.
- Redundant aliases should be dropped when they only repeat a more descriptive inner query signal.
- After ranking, the final stem uses prefix-aware deduplication so words like `month` and `monthly` do not both survive when they are effectively the same signal.
- If parsing fails because the SQL is partial or dialect-specific, the extractor falls back to YAKE instead of hard-failing naming.

## Command Flow

`src-tauri/src/commands/naming.rs` owns the first-save workflow:

1. resolve notes root
2. call `suggest_stem_auto` (auto-detects language when hint is "auto")
3. call `language_to_extension`
4. resolve filename collisions
5. write the file
6. record the save in storage
7. return `SaveResult { path, detected_language }` so FE can update its state

`src-tauri/src/commands/detection.rs` exposes content-based detection:

- `detect_language(content, filename)` — returns a language ID or null

Keep path resolution, collision handling, and filesystem writes in the command layer.

Keep content-based naming logic in `src-tauri/src/naming/`.

## Frontend Flow

`EditorWrapper.svelte` delegates all content-based detection to Rust via IPC:

- `checkLanguage` debounces content changes (1s) and calls `invoke("detect_language")`
- `saveUntitledSlate` passes `languageHint: activeLanguage` — when the hint is "auto", Rust auto-detects
- `syncLanguageFromPath` uses the sync `detectByExtension()` utility for extension-only detection
- `SidebarFileCard` uses `detectByExtension()` for file icon display (no IPC needed)
- `TransformationsPalette` calls `invoke("detect_language")` for selection text detection

The full detection pipeline (structural parsing, heuristic scoring, tree-sitter validation) lives in Rust only. The FE retains only a thin extension map (`detectByExtension.ts`) for sync sidebar rendering.

## Safe Change Checklist

- Add new extension/group routing in `language_profile()`
- Keep new heuristics inside the right extractor module
- Keep the command layer free of language-specific naming rules
- Preserve `suggest_stem` fallback behavior
- When adding tree-sitter support: ensure grammar crate version is compatible with `tree-sitter = "0.24"`
- Add tests close to the module being changed
- Re-run:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `pnpm run check`
  - `pnpm run build`

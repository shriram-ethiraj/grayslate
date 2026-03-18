---
name: naming-architecture
description: Untitled slate naming architecture, language-profile dispatch, and SQL-specific naming heuristics.
---

# Naming Architecture

Use this skill when changing untitled-slate filenames, adding support for a new language or extension, or modifying SQL filename heuristics.

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
- `src/lib/editor/components/EditorWrapper.svelte`

## Rust Module Shape

Rust’s module equivalent is `mod`.

In this area, the naming logic is intentionally organized as a directory module:

- `src-tauri/src/naming/mod.rs` is the public entrypoint.
- sibling files under `src-tauri/src/naming/` hold focused implementation details.

Keep the Tauri command layer importing the stable public functions from `crate::naming` instead of reaching into submodules directly.

## Public API Contract

`src-tauri/src/naming/mod.rs` is the stable surface consumed by commands:

- `suggest_stem(content, language_hint)`
- `language_to_extension(language_hint)`
- `slugify(raw)`
- `fallback_stem()`

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
- new programming language sharing regex extraction behavior → likely `code.rs`
- SQL dialect-specific naming work → `sql.rs`

## Extractor Responsibilities

### `shared.rs`

- bounded input sampling (`MAX_CONTENT_BYTES`)
- slug sanitization
- timestamp fallback naming

This file should stay generic and free of language-specific heuristics.

### `structured.rs`

For delimiter/key-oriented formats:

- CSV
- JSON
- YAML
- TOML

These extractors should stay lightweight and operate on the bounded content sample.

### `markup.rs`

For tree/document formats:

- XML/HTML-like markup
- Markdown

### `code.rs`

For programming languages that can share regex-based symbol extraction through `CodeStyle`.

If multiple extensions want the same symbol-capture behavior, add or reuse a `CodeStyle` variant instead of duplicating regex logic in `mod.rs`.

### `prose.rs`

Fallback keyword extraction using YAKE for unknown or prose-like content.

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
2. call `suggest_stem`
3. call `language_to_extension`
4. resolve filename collisions
5. write the file
6. record the save in storage

Keep path resolution, collision handling, and filesystem writes in the command layer.

Keep content-based naming logic in `src-tauri/src/naming/`.

## Frontend Flow

`EditorWrapper.svelte` does two important things before first save:

- if language mode is still `auto`, it runs a synchronous detection pass at save time
- it sends the resulting `languageHint` into the Rust naming command

That means backend naming should trust the provided hint as the routing input, but should not assume it came from an explicit user choice.

## Safe Change Checklist

- Add new extension/group routing in `language_profile()`
- Keep new heuristics inside the right extractor module
- Keep the command layer free of language-specific naming rules
- Preserve `suggest_stem` fallback behavior
- Add tests close to the module being changed, especially in `sql.rs` for SQL heuristics
- Re-run:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `pnpm run check`
  - `pnpm run build`

---
name: search-architecture
description: Current implementation reference for sidebar library search, Rust search pipeline, frontend wiring, cancellation, and BM25-style ranking. Use when changing search scope, result ranking, preview generation, sidebar query behavior, or the Tauri search command.
---

# Sidebar Search Architecture

This skill documents the current sidebar search implementation in the repository. It reflects the current in-repo search architecture built directly on Rust search crates.

## Primary Files

- `src/lib/components/app-sidebar.svelte`
- `src/lib/files/recentFiles.ts`
- `src-tauri/src/commands/search.rs`
- `src-tauri/src/search/mod.rs`
- `src-tauri/src/search/query.rs`
- `src-tauri/src/search/grep.rs`
- `src-tauri/src/search/scope.rs`
- `src-tauri/src/search/rank.rs`
- `src-tauri/src/search/types.rs`
- `src-tauri/src/filesystem.rs`
- `src-tauri/src/storage.rs`
- `src-tauri/src/lib.rs`

## Current Architecture

### 1. High-Level Pipeline

Sidebar search runs as a three-stage backend pipeline:

1. `query.rs` normalizes the raw string and splits it into whitespace-delimited terms.
2. `scope.rs` resolves the searchable file set from the internal notes root plus tracked external files.
3. `grep.rs` scans file contents and `rank.rs` scores the candidates before returning sorted results.

The orchestration entrypoint is `search::run_sidebar_search()` in `src-tauri/src/search/mod.rs`.

### 2. Crate-Based Search Implementation

- Search is implemented directly with Rust crates:
  - `ignore`
  - `grep-regex`
  - `grep-searcher`
  - `grep-matcher`

This means search behavior is cross-platform through Cargo dependencies and in-process Rust code.

## Frontend Flow

### 1. Sidebar Ownership

- `app-sidebar.svelte` owns the library UI, filter tabs, sort mode, query state, loading states, and empty/error rendering.
- When the query is empty, the sidebar shows recent files from `get_recent_files`.
- When the query is non-empty, the sidebar switches to backend search results from `search_sidebar_files`.

### 2. Search Triggering

- `normalizedQuery` is derived from `query.trim().toLowerCase()`.
- Search requests are debounced by 120 ms in an `$effect`.
- Search only runs while the sidebar is open.
- Clearing the query resets `searchResults`, cancels client-side result acceptance by incrementing the request version, and returns the UI to recent-files mode.

### 3. Frontend Request Contract

`recentFiles.ts` defines the frontend IPC surface:

- `getRecentFiles(limit)` returns `RecentFileRecord[]`
- `searchSidebarFiles(query, filterMode, requestId, limit)` returns `SidebarSearchResult[]`

`SidebarSearchResult` extends the recent file metadata with:

- `preview_line`
- `preview_line_number`
- `match_count`
- `filename_score`
- `content_score`
- `freshness_score`
- `usage_score`
- `final_score`

### 4. Frontend Staleness Handling

- `recentFilesRequestVersion` guards recent-file refreshes.
- `searchRequestVersion` guards live search refreshes.
- If a request resolves after a newer one started, the result is ignored.
- The frontend explicitly suppresses the `Search cancelled.` error string so fast typing does not flash an error state.

## Backend Flow

### 1. Tauri Command Boundary

- `src-tauri/src/commands/search.rs` contains the Tauri command surface only.
- `search_sidebar_files` is async at the IPC boundary and moves the heavy work into `tauri::async_runtime::spawn_blocking`.
- The command clamps the result limit to `1..=200`.
- The command stores per-window cancellation state so a new search cancels the previous in-flight search for the same window.

### 2. Runtime State

`SearchRuntimeState` currently owns:

- a cancellation registry keyed by Tauri window label
- a cached `average_document_length` used by ranking

Do not move ranking logic into the command layer. The current split intentionally keeps retrieval and scoring testable outside of Tauri IPC functions.

### 3. Search Scope Resolution

`scope.rs` constructs `SearchScope`:

- `internal_root`: the configured notes root, if enabled by the current filter mode and if the directory exists
- `external_files`: tracked external files that still exist on disk
- `tracked_by_key`: normalized-path map of all tracked file metadata from SQLite

Filter behavior:

- `unified` searches internal and external files
- `internal` searches only the notes root
- `external` searches only tracked external files

### 4. Candidate Collection

`mod.rs` first builds the candidate path universe:

- file paths discovered by walking the scope
- tracked file paths already known to SQLite

This is intentional. It allows filename-only matches to appear even when content does not match, and it preserves metadata for files already tracked by the app.

### 5. Content Search Implementation

`grep.rs` is the content-matching layer.

- `WalkBuilder` walks the internal notes tree.
- External files are scanned directly from the tracked file list.
- The query is tokenized into terms and each term is searched independently.
- Terms are escaped with `escape_regex_meta()` so matching behaves like fixed-string search.
- `RegexMatcherBuilder::case_smart(true)` preserves smart-case behavior.
- `SearcherBuilder::line_number(true)` captures the first preview line number.
- Binary or unreadable files are skipped by treating `search_path` failures as non-matches.

`ContentMatchSummary` stores:

- `total_hits`
- `term_frequencies`
- `document_frequencies`
- first `preview`

The first matched line becomes the preview returned to the sidebar.

### 6. Ranking Model

`rank.rs` combines multiple signals:

- filename and path heuristics
- BM25-style content score
- freshness from `last_modified_at`
- usage recency from `last_opened_at`, `last_saved_at`, and `last_seen_at`
- pinned-file boost

Current final score formula:

```text
final_score =
  filename_score * 1.6
  + content_score * 1.0
  + freshness_score * 0.15
  + usage_score * 0.1
  + (pinned ? 0.35 : 0.0)
```

Current sort behavior:

- score always dominates first
- `sort_mode` acts as a secondary ordering strategy
- final tiebreakers are lowercase filename, then lowercase full path

### 7. Result Shaping

`mod.rs` merges live filesystem metadata with tracked SQLite metadata to build `FileSearchCandidate` values.

- Live filesystem metadata wins when available for size and modified time.
- Tracked metadata fills gaps for files that are missing or not directly discovered.
- Windows normalized path keys are restored with `restore_normalized_path()` when needed.

## Storage Dependencies

Search depends on SQLite metadata from `storage.rs`.

- `RecentFileRecord` is cloneable so it can be reused across search scope and ranking.
- `list_recent_files()` is still the source of the non-search sidebar mode.
- `list_tracked_files()` is the backend search inventory source.
- `normalize_path_key()` is the canonical path identity function and must stay consistent across file tracking and search.

The current tracked-files query orders by `updated_at DESC` for scope enumeration. Search ranking itself happens later in `rank.rs`.

## Important Invariants

1. Keep search fully in-process and crate-based.
2. Do not add external search-binary resolution into the runtime.
3. Keep Tauri command code in `commands/search.rs` thin.
4. Keep ranking and retrieval code outside command functions.
5. Preserve normalized path behavior across Windows and non-Windows paths.
6. Preserve cancellation checks throughout long-running file walks and scans.
7. Keep frontend stale-result protection aligned with backend cancellation.

## Failure Modes To Watch

- If results flash stale entries while typing, inspect `searchRequestVersion` and the per-window cancellation registry.
- If search stops respecting filter tabs, inspect `scope.rs` first.
- If filename matches disappear, inspect candidate collection in `mod.rs`, not only content scanning.
- If preview lines are missing, inspect `MatchCollector` and `SearcherBuilder::line_number(true)`.
- If cross-platform path handling regresses on Windows, inspect `normalize_path_key()` and `restore_normalized_path()` together.
- If binary files start producing noise, inspect the error-handling path in `grep.rs`.

## Safe Change Checklist

- Update frontend `SidebarSearchResult` typing together with backend response shape changes.
- Re-run `cargo check` after any Rust search-layer change.
- Re-run `pnpm run check` after changing sidebar search UI or request wiring.
- If changing ranking weights, verify both exact filename matches and content-heavy matches still rank sensibly.
- If changing scope rules, verify internal-only, external-only, and unified searches separately.
- If touching docs, keep the language aligned with the crate-based implementation.
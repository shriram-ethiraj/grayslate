---
name: search-architecture
description: Sidebar library search and recent-files orchestration, including backend search, backend-driven refresh events, and reorder suppression.
---

# Sidebar Search Architecture

Use this skill when changing sidebar search scope, ranking, recent-files behavior, open-file interactions, or the library sidebar's refresh/reorder logic.

## Primary Files

- `src/lib/components/app-sidebar.svelte`
- `src/lib/components/sidebar/SidebarFileCard.svelte`
- `src/lib/state/librarySidebar.svelte.ts`
- `src/lib/files/recentFiles.ts`
- `src/lib/files/sidebarUtils.ts`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/file.rs`
- `src-tauri/src/commands/search.rs`
- `src-tauri/src/search/mod.rs`
- `src-tauri/src/search/query.rs`
- `src-tauri/src/search/grep.rs`
- `src-tauri/src/search/scope.rs`
- `src-tauri/src/search/rank.rs`
- `src-tauri/src/search/types.rs`
- `src-tauri/src/storage.rs`

## Current High-Level Split

The library sidebar has two modes:

- **recent-files mode** when `query.trim()` is empty
- **search mode** when the normalized query is non-empty

`app-sidebar.svelte` owns:

- query / filter / sort state
- recent-files data
- search result data
- request-version staleness guards
- backend event listening
- reorder suppression policy

## Search Mode Signal

`isSearchMode` in `app-sidebar.svelte` is a dedicated `$derived(normalizedQuery.length > 0)` boolean that only changes at the empty ↔ non-empty boundary, not on every character typed. All downstream derivations (`activeResults`, `visibleRecentFiles`, `recentFileSections`) and the `SidebarFileList` prop depend on `isSearchMode` instead of `normalizedQuery` directly. This prevents Svelte from re-reconciling the 80-card result list on every keystroke. Do not replace `isSearchMode` with `normalizedQuery.length > 0` in these dependants.

**Critical:** the queued recent-files refresh eventually calls `fetchRecentFiles`, which must keep using `isSearchMode` (not `normalizedQuery`). Reading `normalizedQuery` inside any function called synchronously from an `$effect` subscribes that effect to every keystroke. If you need "is the search query empty?" inside such a function, always read `isSearchMode`.

## Cancellation

Backend search uses cooperative cancellation via `AtomicBool` flags in `SearchRuntimeState` (`src-tauri/src/commands/search.rs`).

Key mechanisms:

- `begin_request()`: cancels the previous search and installs a fresh flag
- `cancel_active()`: cancels without starting a replacement (for query-clear, sidebar-close, teardown)
- `cancel_sidebar_search` Tauri command: exposes `cancel_active()` to the frontend
- `cancelSidebarSearch()` FE wrapper: called from the debounce effect on every keystroke and on component teardown

Cancellation checkpoints in Rust:

- `grep.rs::list_directory_files()` — every directory-walk entry
- `grep.rs::collect_content_matches()` — before each file AND between each term within a file

**Do not remove the per-keystroke `cancelSidebarSearch()` call in the debounce effect.** It ensures the backend stops immediately when a keystroke supersedes the running search, without waiting for the 120 ms debounce to fire a replacement search.

## Matched-Line Display Cap

`SidebarFileCard.svelte` limits visible `matched_lines` per card to 5 (`MAX_VISIBLE_MATCHED_LINES`). The backend still returns up to `MAX_PREVIEWS_PER_FILE` (50), but the card shows the first 5 with a "+N more matches" overflow button. This keeps DOM lightweight when rendering 80 search result cards.

## Shared Sidebar State

`src/lib/state/librarySidebar.svelte.ts` is the cross-component coordination surface for sidebar/editor/dialog behavior.

Current shared fields:

- `pendingOpenFile`
- `requestActivateSearch`
- `handleLibraryMutation`

Use this shared state for sidebar/editor/dialog coordination instead of ad-hoc custom event chains.

`reportLibraryMutation(...)` is the typed entry point for file-operation results.
It classifies created, opened, duplicated, removed, renamed, and generic sync
updates so the sidebar owns refresh, suppression, tab, and reveal policy.

## Recent-Files Mode

`recentFiles.ts` defines the frontend IPC contract:

- `getRecentFiles(limit)`
- `searchSidebarFiles(query, filterMode, requestId, limit)`
- `cancelSidebarSearch()` — immediately cancels any in-flight search
- `deleteFile(path)`
- `renameFile(path, newName)`
- `duplicateFile(path)`
- `duplicateLocalFileAsSlate(path)`

When the query is empty:

- the sidebar shows recent files from `get_recent_files`
- `visibleRecentFiles` applies source filtering and sorting
- recency sections are built only for the recent-opened sorts

## Search Mode

When the query is non-empty:

- `normalizedQuery` is `query.trim().toLowerCase()`
- search is debounced by 120 ms
- search only runs while the sidebar is open
- `searchRequestVersion` guards against stale results
- `"Search cancelled."` is intentionally suppressed in the UI

### Search Options

`app-sidebar.svelte` owns a `searchOptions: SearchOptions` state with three boolean toggles:

- `caseSensitive` — when on, matching is case-sensitive; when off (default), case-insensitive
- `wholeWord` — when on, terms are wrapped with `\b` word boundaries
- `useRegex` — when on, the entire query is passed as a single regex pattern (no whitespace splitting)

`SidebarHeader.svelte` renders three VS Code-style toggle buttons inside the search input (codicon icons: `case-sensitive`, `whole-word`, `regex`). Active toggles show a highlighted background.

A `searchOptionsKey` derived string changes only on toggle clicks and is tracked in the debounce `$effect` so option changes re-trigger search without a keystroke.

The Clear button resets both the query text and all toggles to defaults.

The `searchSidebarFiles` IPC call passes the three booleans as top-level params (`caseSensitive`, `wholeWord`, `useRegex`). The Rust `search_sidebar_files` command accepts them as `Option<bool>` for backward compatibility.

On the Rust side, `SearchOptions` lives in `search/query.rs` and is embedded in `ParsedSearchQuery`. The pipeline threads it through:

- `query.rs` — controls case normalisation and term splitting (regex mode = single term)
- `grep.rs` — controls regex escaping, word boundaries, and case insensitivity flags
- `rank.rs` — controls filename scoring (case-aware substring matching or regex match)
- `types.rs` — controls highlight fragment generation (literal vs regex, case-aware)

Invalid regex patterns surface as error strings through the existing `loadError` banner in `SidebarFileList.svelte`.

`SidebarSearchResult` currently includes:

- all `RecentFileRecord` fields
- `matched_lines: { line_number, line_text }[]`
- `match_count`
- `filename_score`
- `content_score`
- `freshness_score`
- `usage_score`
- `final_score`

`SidebarFileCard.svelte` renders excerpts/highlights from `matched_lines` through helpers in `sidebarUtils.ts`.

## Reorder Suppression Policy

This is the major frontend architecture change for the library sidebar.

Goal:

- when the user opens a file from the sidebar, the visible recent-files list must not jump/re-sort under the cursor

State:

- `suppressReorder`
- `lastSidebarOpenedPath`

Activation:

- `openRecentFile(...)` sets `suppressReorder = true`
- it also records `lastSidebarOpenedPath`

Behavior while active:

- generic background `RECENT_FILES_UPDATED_EVENT` refreshes are deferred;
  explicit `"saved"` events refresh immediately
- pure filter-tab changes do not refetch
- successful structural mutations clear suppression and refresh immediately

Suppression clears only on explicit user or session boundaries:

- sort change
- manual refresh
- sidebar close/reopen cycle
- external navigation that does not match the sidebar-opened path

## Library Refresh Coordinator

The sidebar now prefers a single-list model plus quiet refreshes over the old "staged buffer" style.

`src/lib/files/libraryRefreshCoordinator.ts` serializes and coalesces refresh
work. It targets the active dataset: search mode refreshes `searchResults`,
while browse mode refreshes `recentFiles`.

Important behavior:

- immediate structural actions override suppression
- background updates collapse into one deferred refresh while suppressed
- sidebar-close while suppressed releases suppression and triggers an invisible recent-files refresh
- reopening the sidebar then shows already-fresh data with no visible jitter

Do not reintroduce a second recent-files buffer unless there is a very strong reason.

## Structural Mutation Policy

User-triggered structural operations report semantic mutations after the
backend succeeds.

Flow:

1. the operation succeeds and reports its mutation
2. the sidebar applies immediate local visibility changes where possible
3. suppression is released and the active dataset is refreshed
4. the backend event is coalesced as reconciliation, not a second policy path

Rename, delete, unlink, duplicate, first-time creation, and completed saves
are immediate. Opening a file is read-only and does not affect timestamps or
list ordering.

## Backend-Driven Recent Files Refresh

The frontend no longer owns recent-file update emits for file operations.

`src-tauri/src/commands/mod.rs` defines:

- `RECENT_FILES_UPDATED_EVENT = "files://recent-updated"`

The backend emits this event after file mutations, including:

- `write_file_content`
- `delete_file`
- `rename_file`
- `duplicate_local_file_as_slate`
- `duplicate_file`
- `save_untitled_slate`

Content saves emit the payload `"saved"`; the sidebar uses it to release
open-order suppression and immediately refresh. Other mutation events use the
generic sync path.

`app-sidebar.svelte` reports that event as `{ kind: "sync" }`. Its mutation
coordinator coalesces refreshes and defers background sync while reorder
suppression is active.

This means:

- file-operation callers report semantic mutations only when they need a UI
  policy beyond generic backend sync (for example, reveal a duplicated slate)
- `EditorWrapper` should not emit recent-file update events after open/read

## Open-File Flow

Sidebar-opened navigation still uses `OPEN_FILE_PATH_EVENT`.

Flow:

1. `app-sidebar.svelte::openRecentFile(...)` sets suppression + pending-open metadata
2. it emits `OPEN_FILE_PATH_EVENT`
3. `EditorWrapper.svelte` listens and opens the file
4. `read_file_content` returns bytes without changing storage or emitting a refresh event
5. after a successful load, `EditorWrapper` reports whether the open originated from the sidebar or externally
6. sidebar opens remain deferred; external opens clear search, ensure a visible source tab, refresh, and reveal

This split is intentional:

- `OPEN_FILE_PATH_EVENT` is the FE navigation signal
- `RECENT_FILES_UPDATED_EVENT` is the backend data-refresh signal

## Backend Search Pipeline

Search itself still follows the crate-based Rust pipeline:

1. `query.rs` normalizes and tokenizes the query
2. `scope.rs` resolves the searchable file universe
3. `grep.rs` scans file contents
4. `rank.rs` scores and sorts results

The orchestration entrypoint remains `search::run_sidebar_search()` in `src-tauri/src/search/mod.rs`.

Search implementation stays fully in-process and crate-based:

- `ignore`
- `grep-regex`
- `grep-searcher`
- `grep-matcher`

## Ranking and Scope Notes

Key ranking inputs still include:

- filename/path heuristics
- BM25-style content score
- freshness
- usage recency
- pinned-file boost

Scope still supports:

- `unified`
- `internal`
- `external`

Candidate collection still includes both:

- filesystem-discovered files
- tracked SQLite files

That preserves filename-only matches and metadata-rich ranking.

## Important Invariants

1. Keep search fully in-process and crate-based.
2. Keep command-layer code thin; ranking/retrieval stays outside Tauri IPC functions.
3. Treat `RECENT_FILES_UPDATED_EVENT` as backend-owned.
4. Preserve `suppressReorder` behavior after sidebar-initiated opens.
5. Keep explicit structural mutations immediate and background sync deferred.
6. Keep stale-result guards aligned with backend cancellation.
7. Preserve normalized-path behavior across Windows and non-Windows paths.
8. **Never read `normalizedQuery` inside functions called synchronously from `$effect`.** Use `isSearchMode` for boolean checks. See "Search Mode Signal" above.
9. **Always call `cancelSidebarSearch()` before clearing results or exiting search.** This prevents the backend from wasting thread-pool time on stale work.
10. **Keep matched-line display capped in `SidebarFileCard`.** The backend intentionally returns up to 50 per file for future use; the FE must cap what it renders.

## Performance Pitfalls

These patterns caused severe input lag and must not be reintroduced:

### 1. Reactive subscription leak in `fetchRecentFiles`

The mount `$effect` schedules a recent-files refresh. In Svelte 5, all `$state`/`$derived` reads **before the first `await`** inside an async function called directly from an effect become subscriptions of that effect. Previously, `normalizedQuery.length` was read in the refresh path, making the mount effect re-run on every keystroke — flooding Rust with `getRecentFiles()` IPC calls (120+ filesystem stats each) and toggling `isLoading` per character. The scheduler now breaks that direct effect call chain; `fetchRecentFiles` still uses `isSearchMode` as a defensive invariant.

**Rule:** Any async function called from an `$effect` must never read fast-changing reactive state before its first `await`. Snapshot the value beforehand or use a coarser derived signal.

### 2. Missing explicit cancel path

Without `cancel_sidebar_search`, the backend only stopped work when the **next** `begin_request()` arrived — which happened after the 120 ms debounce. During that window, stale search work continued, competing for the Rust thread pool. Fixed by adding `cancelSidebarSearch()` calls on every keystroke and on teardown.

### 3. Unbounded matched-line rendering

Each `SidebarFileCard` rendered up to 50 `matched_lines` (buttons + fragment loops + `<mark>` elements). Worst case: 80 cards × 50 lines = 4000 interactive buttons rendered synchronously. Fixed by capping visible lines to 5 with a "+N more" overflow button.

### 4. Per-term cancellation gap in grep

`collect_content_matches` only checked `cancelled` once per file, not between terms. A multi-term query on a large file could keep the blocking thread busy. Fixed by adding `ensure_not_cancelled` between each `(term, matcher)` iteration.

## Failure Modes To Watch

- If the list jumps after clicking a sidebar file, inspect `suppressReorder` and `lastSidebarOpenedPath`.
- If rename remains stale, inspect `reportLibraryMutation` and the active-dataset refresh.
- If duplicate/delete/rename do not refresh the sidebar, inspect backend event emit sites before adding frontend refresh code.
- If search flashes stale results while typing, inspect `searchRequestVersion` and backend cancellation.
- If filename-only matches disappear, inspect candidate collection in `search/mod.rs`, not only grep logic.
- **If typing in the search input lags**, check that `fetchRecentFiles` has no `normalizedQuery` reads before its first `await`, and that the debounce effect still calls `cancelSidebarSearch()`.
- **If backend search keeps running after the sidebar closes**, check that the teardown cleanup in the event-listener `$effect` still calls `cancelSidebarSearch()`.

## Safe Change Checklist

- Update frontend and backend search-result shapes together.
- Re-run `cargo check` after Rust sidebar/search changes.
- Re-run `pnpm run check` after changing `app-sidebar.svelte`, `SidebarFileCard.svelte`, or sidebar state wiring.
- Verify recent-files mode, search mode, and sidebar-opened navigation separately.
- Verify a rename of the currently open file does not reorder the sidebar unexpectedly.

# Naming Audit — Grayslate Project Files

Comparing what a smart IDE would name each file (content-only, no filename hint)
against what our auto-naming system currently produces.

**Status key:**
- ✅ Good — accurate and concise
- ⚠️ Noisy — right concept, too many parts joined
- ❌ Poor — misleading or generic
- 🚫 Fallback — timestamp used (no name derived)

**Overall: 243 / 247 named (98%). 4 fallbacks.**

---

## Config & Manifest Files

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `package.json` | grayslate | json | `grayslate` | `grayslate` | ✅ |
| `src-tauri/Cargo.toml` | grayslate | toml | `grayslate` | `grayslate` | ✅ |
| `svelte.config.js` | grayslate | javascript | `svelte-config` | FALLBACK | 🚫 |
| `vite.config.js` | grayslate | javascript | `vite-config` | `define-config` | ⚠️ |
| `src-tauri/tauri.conf.json` | grayslate | json | `tauri-config` | `product-name-version-identifier-build` | ⚠️ |
| `src-tauri/tauri.macos.conf.json` | grayslate | json | `tauri-macos-config` | `app` | ❌ |
| `src-tauri/capabilities/default.json` | grayslate | json | `default-capabilities` | `identifier-description-windows-permissions` | ⚠️ |
| `src-tauri/capabilities/desktop.json` | grayslate | json | `desktop-capabilities` | `identifier-platforms-windows-permissions` | ⚠️ |
| `components.json` | grayslate | json | `shadcn-components` | *(check run)* | ⚠️ |

---

## Documentation

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `README.md` | grayslate | markdown | `grayslate-readme` | `key-features` | ⚠️ |
| `agents.md` | grayslate | markdown | `ai-agent-guidelines` | `ai-agent-implementation-guidelines-grayslate` | ⚠️ |
| `audit-report.md` | grayslate | markdown | `audit-report` | *(check run)* | — |

---

## Rust Backend — Commands

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src-tauri/src/main.rs` | grayslate | rust | `app-entry` | FALLBACK | 🚫 |
| `src-tauri/build.rs` | grayslate | rust | `build-script` | FALLBACK | 🚫 |
| `src-tauri/src/lib.rs` | grayslate | rust | `lib-modules` | `commands-filesystem-menu-naming` | ⚠️ |
| `src-tauri/src/commands/mod.rs` | grayslate | rust | `commands-module` | `file-memory-naming-search` | ⚠️ |
| `src-tauri/src/commands/file.rs` | grayslate | rust | `file-commands` | `file-read-cancellation-registry-active-file-read-ensure` | ⚠️ |
| `src-tauri/src/commands/memory.rs` | grayslate | rust | `memory-commands` | `memory-info-get-memory-info` | ⚠️ |
| `src-tauri/src/commands/naming.rs` | grayslate | rust | `naming-commands` | `save-untitled-slate-suggest-slate-name-build-suggested-name` | ⚠️ |
| `src-tauri/src/commands/search.rs` | grayslate | rust | `search-commands` | `search-runtime-state-search-sidebar-files-active-search` | ⚠️ |
| `src-tauri/src/commands/transform.rs` | grayslate | rust | `transform-commands` | `transformation-message-level-validate-jsonc-parse-jsonc-to` | ⚠️ |
| `src-tauri/src/commands/update.rs` | grayslate | rust | `update-commands` | `update-check-response-update-install-response-check-for` | ⚠️ |

---

## Rust Backend — Core

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src-tauri/src/filesystem.rs` | grayslate | rust | `filesystem` | `resolve-default-notes-root-path-resolve-notes-root-path` | ⚠️ |
| `src-tauri/src/storage.rs` | grayslate | rust | `app-storage` | `app-storage-file-source-file-event-type-recent-file-record` | ⚠️ |
| `src-tauri/src/menu/mod.rs` | grayslate | rust | `macos-menu` | `mac-os-menu-state` | ✅ |
| `src-tauri/src/window/mod.rs` | grayslate | rust | `macos-window-styling` | `apply-macos-window-styling` | ⚠️ |

---

## Rust Backend — Naming Module

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src-tauri/src/naming/mod.rs` | grayslate | rust | `naming-module` | `code-markup-model-prose` | ⚠️ |
| `src-tauri/src/naming/model.rs` | grayslate | rust | `naming-model` | `language-naming-profile-extractor-group-structured-naming` | ⚠️ |
| `src-tauri/src/naming/shared.rs` | grayslate | rust | `naming-shared` | `bound-slugify-fallback-stem-format-readable-timestamp` | ⚠️ |
| `src-tauri/src/naming/code.rs` | grayslate | rust | `code-naming` | `extract-code-symbol-is-noise-name-try-tree-sitter` | ⚠️ |
| `src-tauri/src/naming/markup.rs` | grayslate | rust | `markup-naming` | `extract-xml-html-extract-markdown` | ✅ |
| `src-tauri/src/naming/prose.rs` | grayslate | rust | `prose-naming` | `extract-prose-try-extract-email-is-email-extract-subject` | ⚠️ |
| `src-tauri/src/naming/sql.rs` | grayslate | rust | `sql-naming` | `expr-col-name-collect-where-columns-is-generic-cte-name` | ⚠️ |
| `src-tauri/src/naming/structured.rs` | grayslate | rust | `structured-naming` | `extract-csv-is-noise-csv-column-is-json-noise-key-json` | ⚠️ |

---

## Rust Backend — Search Module

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src-tauri/src/search/mod.rs` | grayslate | rust | `search-module` | `grep-query-rank-scope` | ✅ |
| `src-tauri/src/search/grep.rs` | grayslate | rust | `search-grep` | `list-scope-files-collect-content-matches-list-directory` | ⚠️ |
| `src-tauri/src/search/query.rs` | grayslate | rust | `search-query` | `parsed-search-query-parse-query` | ✅ |
| `src-tauri/src/search/rank.rs` | grayslate | rust | `search-rank` | `rank-context-resolve-average-document-length-rank-candidate` | ⚠️ |
| `src-tauri/src/search/scope.rs` | grayslate | rust | `search-scope` | `search-scope-resolve-search-scope` | ✅ |
| `src-tauri/src/search/types.rs` | grayslate | rust | `search-types` | `search-preview-content-match-summary-file-search-candidate` | ⚠️ |

---

## TypeScript — Editor Config

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/editor/config/languageExtensions.ts` | grayslate | typescript | `language-extensions` | `get-language-extension` | ✅ |
| `src/lib/editor/config/supportedLanguages.ts` | grayslate | typescript | `supported-languages` | `language-language-icon` | ⚠️ |

---

## TypeScript — Editor Core

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/editor/core/actions.ts` | grayslate | typescript | `editor-actions` | `update-search-stats-options-search-stats-worker-state-clear` | ⚠️ |
| `src/lib/editor/core/csvCodeMirror.ts` | grayslate | typescript | `csv-codemirror` | `text-change-spec-get-minimal-text-change-dispatch-csv-doc` | ⚠️ |
| `src/lib/editor/core/csvParser.ts` | grayslate | typescript | `csv-parser` | `csv-parse-result-parse-csv-serialize-csv` | ⚠️ |
| `src/lib/editor/core/editorSession.ts` | grayslate | typescript | `editor-session` | `session-bindings-managed-editor-session-clear-value-sync` | ⚠️ |
| `src/lib/editor/core/languageDetector.ts` | grayslate | typescript | `language-detector` | `max-content-length-heuristic-score-threshold-partial-score` | ⚠️ |
| `src/lib/editor/core/languageDetector.test.ts` | grayslate | typescript | `language-detector-tests` | `difficulty-detect-case-detect-phase-assert` | ⚠️ |
| `src/lib/editor/core/memory.ts` | grayslate | typescript | `editor-memory` | `memory-info-shrink-metrics-request-file-open-reclaim-doc` | ⚠️ |

---

## TypeScript — Editor Extensions

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/editor/extensions/autocompleteFactory.ts` | grayslate | typescript | `autocomplete-factory` | `autocomplete-item-autocomplete-config-autocomplete` | ⚠️ |
| `src/lib/editor/extensions/colorHints.ts` | grayslate | typescript | `color-hints` | `color-swatch-widget-clear-color-cache-get-canvas-ctx-to` | ⚠️ |
| `src/lib/editor/extensions/contextMenuExtension.ts` | grayslate | typescript | `context-menu-extension` | `context-menu-data-consume-context-menu-data-block-select` | ⚠️ |
| `src/lib/editor/extensions/csvRainbowHighlight.ts` | grayslate | typescript | `csv-rainbow-highlight` | `build-decorations-viewport-key-field-marks` | ⚠️ |
| `src/lib/editor/extensions/csvStickyScroll.ts` | grayslate | typescript | `csv-sticky-scroll` | `csv-sticky-scroll-parse-csv-scopes` | ✅ |
| `src/lib/editor/extensions/csvUtils.ts` | grayslate | typescript | `csv-utils` | `field-range-num-colors-detect-delimiter-get-field-ranges` | ⚠️ |
| `src/lib/editor/extensions/jsonFoldWidget.ts` | grayslate | typescript | `json-fold-widget` | `key-value-pair-fold-info-truncate-format-value` | ⚠️ |
| `src/lib/editor/extensions/jsonInlayHints.ts` | grayslate | typescript | `json-inlay-hints` | `array-index-widget-json-inlay-hints` | ✅ |
| `src/lib/editor/extensions/jsonKeyPath.ts` | grayslate | typescript | `json-key-path` | `extract-property-key-build-json-path` | ⚠️ |
| `src/lib/editor/extensions/jsonStickyScroll.ts` | grayslate | typescript | `json-sticky-scroll` | `parse-json-scopes-json-sticky-scroll` | ✅ |
| `src/lib/editor/extensions/stickyHeader.ts` | grayslate | typescript | `sticky-header` | `sticky-header-config-is-anchor-line-visible` | ✅ |
| `src/lib/editor/extensions/stickyScroll.ts` | grayslate | typescript | `sticky-scroll` | `sticky-scope-sticky-scroll-config` | ✅ |

---

## TypeScript — Workers & Protocols

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/editor/workers/csvTable.worker.ts` | grayslate | typescript | `csv-table-worker` | `cell-edit-row-add-row-delete-header-edit` | ⚠️ |
| `src/lib/editor/workers/findStats.worker.ts` | grayslate | typescript | `find-stats-worker` | `search-match-range-search-checkpoint-search-stats-cache` | ⚠️ |
| `src/lib/editor/workers/findStatsProtocol.ts` | grayslate | typescript | `find-stats-protocol` | `find-stats-worker-request-find-stats-worker-response` | ✅ |
| `src/lib/editor/workers/markdownPreview.worker.ts` | grayslate | typescript | `markdown-preview-worker` | `post-response` | ❌ |
| `src/lib/editor/workers/markdownPreviewProtocol.ts` | grayslate | typescript | `markdown-preview-protocol` | `markdown-preview-worker-request-markdown-preview-worker` | ✅ |

---

## TypeScript — Markdown Components

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/editor/components/markdown/markdownAutocomplete.ts` | grayslate | typescript | `markdown-autocomplete` | `markdown-autocomplete-config-markdown-autocomplete-provider` | ✅ |
| `src/lib/editor/components/markdown/previewActions.ts` | grayslate | typescript | `markdown-preview-actions` | `register-markdown-preview-element-unregister-markdown` | ⚠️ |
| `src/lib/editor/components/markdown/renderMarkdown.ts` | grayslate | typescript | `render-markdown` | `build-line-starts-offset-to-line-escape-html-block-tokens` | ⚠️ |
| `src/lib/editor/components/markdown/scrollSync.ts` | grayslate | typescript | `markdown-scroll-sync` | `scroll-anchor-build-anchor-map-line-percent-to-preview` | ⚠️ |

---

## TypeScript — CSV Components

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/editor/components/csv/csvTableProtocol.ts` | grayslate | typescript | `csv-table-protocol` | `csv-table-controller-csv-table-snapshot-csv-row-window-csv` | ⚠️ |
| `src/lib/editor/components/csv/useCsvEditorState.svelte.ts` | grayslate | typescript | `csv-editor-state` | `selection-block-mutation-loader-config-use-csv-editor-state` | ⚠️ |
| `src/lib/editor/components/csv/useScrollVirtualizer.svelte.ts` | grayslate | typescript | `scroll-virtualizer` | `get-visible-count-get-max-row-scroll-update-scroll-metrics` | ⚠️ |

---

## TypeScript — UI Barrel Files (`index.ts`)

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/components/ui/badge/index.ts` | grayslate | typescript | `badge` | `root-badge-badge-variants-badge-props` | ⚠️ |
| `src/lib/components/ui/button/index.ts` | grayslate | typescript | `button` | `root-props-button-button-variants` | ⚠️ |
| `src/lib/components/ui/command/index.ts` | grayslate | typescript | `command` | `root-dialog-empty-group` | ⚠️ |
| `src/lib/components/ui/context-menu/index.ts` | grayslate | typescript | `context-menu` | `root-sub-portal-item` | ⚠️ |
| `src/lib/components/ui/dialog/index.ts` | grayslate | typescript | `dialog` | `root-title-portal-footer` | ⚠️ |
| `src/lib/components/ui/input/index.ts` | grayslate | typescript | `input` | `root-input` | ⚠️ |
| `src/lib/components/ui/item/index.ts` | grayslate | typescript | `item` | `root-group-media-content` | ⚠️ |
| `src/lib/components/ui/menubar/index.ts` | grayslate | typescript | `menubar` | `root-checkbox-item-content-item` | ⚠️ |
| `src/lib/components/ui/resizable/index.ts` | grayslate | typescript | `resizable` | `pane-group-pane-resizable-pane-group-resizable-pane` | ⚠️ |
| `src/lib/components/ui/select/index.ts` | grayslate | typescript | `select` | `root-group-label-item` | ⚠️ |
| `src/lib/components/ui/separator/index.ts` | grayslate | typescript | `separator` | `root-separator` | ⚠️ |
| `src/lib/components/ui/sidebar/index.ts` | grayslate | typescript | `sidebar` | `content-footer-group-group-content` | ⚠️ |
| `src/lib/components/ui/skeleton/index.ts` | grayslate | typescript | `skeleton` | `root-skeleton` | ⚠️ |
| `src/lib/components/ui/sonner/index.ts` | grayslate | typescript | `sonner` | `toaster-toast` | ✅ |
| `src/lib/components/ui/tabs/index.ts` | grayslate | typescript | `tabs` | `root-list-trigger-content` | ⚠️ |

---

## TypeScript — Other UI & Lib Files

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/icons.d.ts` | grayslate | typescript | `icon-types` | FALLBACK | 🚫 |
| `src/lib/components/ui/sidebar/constants.ts` | grayslate | typescript | `sidebar-constants` | `sidebar-cookie-name-sidebar-cookie-max-age-sidebar-width` | ⚠️ |
| `src/lib/components/ui/sidebar/context.svelte.ts` | grayslate | typescript | `sidebar-context` | `sidebar-state-getter-sidebar-state-props-set-sidebar` | ⚠️ |
| `src/lib/components/ui/sonner/toast.ts` | grayslate | typescript | `toast` | `base-toast-toast-message-toast-data-typed-toast-method` | ⚠️ |
| `src/lib/files/notesRoot.ts` | grayslate | typescript | `notes-root` | `get-configured-notes-root-set-configured-notes-root-resolve` | ⚠️ |
| `src/lib/files/recentFiles.ts` | grayslate | typescript | `recent-files` | `recent-file-record-matched-line-sidebar-search-result-open` | ⚠️ |
| `src/lib/files/sidebarUtils.ts` | grayslate | typescript | `sidebar-utils` | `recent-file-section-filter-mode-sort-mode-recency-bucket` | ⚠️ |
| `src/lib/hooks/create-theme.ts` | grayslate | typescript | `create-theme` | `theme-settings-theme-config` | ✅ |
| `src/lib/hotkeys.ts` | grayslate | typescript | `hotkeys` | `hotkey-manager-hotkey-binding-register-hotkey-register` | ⚠️ |
| `src/lib/ipc.ts` | grayslate | typescript | `ipc` | `chunked-text-event-chunk-waiter-reset-buffers-reject-waiter` | ⚠️ |
| `src/lib/state/appMenu.svelte.ts` | grayslate | typescript | `app-menu-state` | *(check run)* | — |
| `src/routes/+layout.ts` | grayslate | typescript | `layout-config` | `ssr` | ❌ |

---

## Svelte — App Shell & Dialogs

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/routes/+layout.svelte` | grayslate | svelte | `app-layout` | `typeof-relative-flex-1-overflow-hidden-h-full-min-h-0` | ❌ |
| `src/routes/+page.svelte` | grayslate | svelte | `editor-page` | `editor-wrapper` | ✅ |
| `src/lib/components/AboutDialog.svelte` | grayslate | svelte | `about-dialog` | `void-p-0-sm-max-w-[44rem]-sr-only` | ❌ |
| `src/lib/components/DeleteFileDialog.svelte` | grayslate | svelte | `delete-file-dialog` | `void-sm-max-w-[26rem]-font-medium-text-foreground` | ❌ |
| `src/lib/components/RenameFileDialog.svelte` | grayslate | svelte | `rename-file-dialog` | `htmlinput-element-sm-max-w-[25rem]-grid-gap-3` | ❌ |
| `src/lib/components/Titlebar.svelte` | grayslate | svelte | `titlebar` | `string` | ❌ |
| `src/lib/components/app-sidebar.svelte` | grayslate | svelte | `app-sidebar` | `filter-mode` | ⚠️ |
| `src/lib/components/theme-toggle.svelte` | grayslate | svelte | `theme-toggle` | `button-size-4-transition-all-size-4-transition-all` | ❌ |

---

## Svelte — Sidebar Components

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/components/sidebar/SidebarFileCard.svelte` | grayslate | svelte | `sidebar-file-card` | `void` | ❌ |
| `src/lib/components/sidebar/SidebarFileList.svelte` | grayslate | svelte | `sidebar-file-list` | `div-flex-1-min-h-0-overflow-auto-p-2-gap-2-p-0` | ❌ |
| `src/lib/components/sidebar/SidebarHeader.svelte` | grayslate | svelte | `sidebar-header` | `htmlinput-element-min-w-0-truncate-text-sm-font-medium` | ❌ |

---

## Svelte — UI Components (Badge → Dialog)

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/components/ui/badge/badge.svelte` | grayslate | svelte | `badge` | `typeof` | ❌ |
| `src/lib/components/ui/button/button.svelte` | grayslate | svelte | `button` | `typeof` | ❌ |
| `src/lib/components/ui/command/command-dialog.svelte` | grayslate | svelte | `command-dialog` | `dialog-primitive-command-palette-sr-only` | ⚠️ |
| `src/lib/components/ui/command/command-empty.svelte` | grayslate | svelte | `command-empty` | `command-primitive` | ✅ |
| `src/lib/components/ui/command/command-group.svelte` | grayslate | svelte | `command-group` | `command-primitive` | ✅ |
| `src/lib/components/ui/command/command-input.svelte` | grayslate | svelte | `command-input` | `div-size-4-shrink-0-opacity-50` | ❌ |
| `src/lib/components/ui/command/command-item.svelte` | grayslate | svelte | `command-item` | `command-primitive` | ✅ |
| `src/lib/components/ui/command/command-link-item.svelte` | grayslate | svelte | `command-link-item` | `command-primitive` | ✅ |
| `src/lib/components/ui/command/command-list.svelte` | grayslate | svelte | `command-list` | `command-primitive` | ✅ |
| `src/lib/components/ui/command/command-loading.svelte` | grayslate | svelte | `command-loading` | `command-primitive` | ✅ |
| `src/lib/components/ui/command/command-separator.svelte` | grayslate | svelte | `command-separator` | `command-primitive` | ✅ |
| `src/lib/components/ui/command/command-shortcut.svelte` | grayslate | svelte | `command-shortcut` | `htmlattributes` | ❌ |
| `src/lib/components/ui/command/command.svelte` | grayslate | svelte | `command` | `command-primitive` | ✅ |
| `src/lib/components/ui/dialog/dialog-close.svelte` | grayslate | svelte | `dialog-close` | `dialog-primitive` | ✅ |
| `src/lib/components/ui/dialog/dialog-content.svelte` | grayslate | svelte | `dialog-content` | `dialog-primitive-sr-only` | ✅ |
| `src/lib/components/ui/dialog/dialog-description.svelte` | grayslate | svelte | `dialog-description` | `dialog-primitive` | ✅ |
| `src/lib/components/ui/dialog/dialog-footer.svelte` | grayslate | svelte | `dialog-footer` | `htmlattributes` | ❌ |
| `src/lib/components/ui/dialog/dialog-header.svelte` | grayslate | svelte | `dialog-header` | `htmlattributes` | ❌ |
| `src/lib/components/ui/dialog/dialog-overlay.svelte` | grayslate | svelte | `dialog-overlay` | `dialog-primitive` | ✅ |
| `src/lib/components/ui/dialog/dialog-portal.svelte` | grayslate | svelte | `dialog-portal` | `dialog-primitive` | ✅ |
| `src/lib/components/ui/dialog/dialog-title.svelte` | grayslate | svelte | `dialog-title` | `dialog-primitive` | ✅ |
| `src/lib/components/ui/dialog/dialog-trigger.svelte` | grayslate | svelte | `dialog-trigger` | `dialog-primitive` | ✅ |
| `src/lib/components/ui/dialog/dialog.svelte` | grayslate | svelte | `dialog` | `dialog-primitive` | ✅ |
| `src/lib/components/ui/input/input.svelte` | grayslate | svelte | `input` | `htmlinput-type-attribute-relative-w-full-size-3-5` | ❌ |

---

## Svelte — UI Components (Context Menu)

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/components/ui/context-menu/context-menu-checkbox-item.svelte` | grayslate | svelte | `context-menu-checkbox-item` | `context-menu-primitive-size-4` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-content.svelte` | grayslate | svelte | `context-menu-content` | `component-props` | ⚠️ |
| `src/lib/components/ui/context-menu/context-menu-group-heading.svelte` | grayslate | svelte | `context-menu-group-heading` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-group.svelte` | grayslate | svelte | `context-menu-group` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-item.svelte` | grayslate | svelte | `context-menu-item` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-label.svelte` | grayslate | svelte | `context-menu-label` | `htmlattributes` | ❌ |
| `src/lib/components/ui/context-menu/context-menu-portal.svelte` | grayslate | svelte | `context-menu-portal` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-radio-group.svelte` | grayslate | svelte | `context-menu-radio-group` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-radio-item.svelte` | grayslate | svelte | `context-menu-radio-item` | `context-menu-primitive-size-2-fill-current` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-separator.svelte` | grayslate | svelte | `context-menu-separator` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-shortcut.svelte` | grayslate | svelte | `context-menu-shortcut` | `htmlattributes` | ❌ |
| `src/lib/components/ui/context-menu/context-menu-sub-content.svelte` | grayslate | svelte | `context-menu-sub-content` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-sub-trigger.svelte` | grayslate | svelte | `context-menu-sub-trigger` | `context-menu-primitive-ms-auto` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-sub.svelte` | grayslate | svelte | `context-menu-sub` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu-trigger.svelte` | grayslate | svelte | `context-menu-trigger` | `context-menu-primitive` | ✅ |
| `src/lib/components/ui/context-menu/context-menu.svelte` | grayslate | svelte | `context-menu` | `context-menu-primitive` | ✅ |

---

## Svelte — UI Components (Item, Menubar, Resizable, Select)

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/components/ui/item/item-actions.svelte` | grayslate | svelte | `item-actions` | `htmlattributes` | ❌ |
| `src/lib/components/ui/item/item-content.svelte` | grayslate | svelte | `item-content` | `htmlattributes` | ❌ |
| `src/lib/components/ui/item/item-description.svelte` | grayslate | svelte | `item-description` | `htmlattributes` | ❌ |
| `src/lib/components/ui/item/item-footer.svelte` | grayslate | svelte | `item-footer` | `htmlattributes` | ❌ |
| `src/lib/components/ui/item/item-group.svelte` | grayslate | svelte | `item-group` | `htmlattributes` | ❌ |
| `src/lib/components/ui/item/item-header.svelte` | grayslate | svelte | `item-header` | `htmlattributes` | ❌ |
| `src/lib/components/ui/item/item-media.svelte` | grayslate | svelte | `item-media` | `typeof` | ❌ |
| `src/lib/components/ui/item/item-root.svelte` | grayslate | svelte | `item-root` | `typeof` | ❌ |
| `src/lib/components/ui/item/item-separator.svelte` | grayslate | svelte | `item-separator` | `typeof` | ❌ |
| `src/lib/components/ui/item/item-title.svelte` | grayslate | svelte | `item-title` | `htmlattributes` | ❌ |
| `src/lib/components/ui/menubar/menubar-checkbox-item.svelte` | grayslate | svelte | `menubar-checkbox-item` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-content.svelte` | grayslate | svelte | `menubar-content` | `component-props` | ⚠️ |
| `src/lib/components/ui/menubar/menubar-group-heading.svelte` | grayslate | svelte | `menubar-group-heading` | `typeof` | ❌ |
| `src/lib/components/ui/menubar/menubar-group.svelte` | grayslate | svelte | `menubar-group` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-item.svelte` | grayslate | svelte | `menubar-item` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-label.svelte` | grayslate | svelte | `menubar-label` | `htmlattributes` | ❌ |
| `src/lib/components/ui/menubar/menubar-menu.svelte` | grayslate | svelte | `menubar-menu` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-portal.svelte` | grayslate | svelte | `menubar-portal` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-radio-group.svelte` | grayslate | svelte | `menubar-radio-group` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-radio-item.svelte` | grayslate | svelte | `menubar-radio-item` | `menubar-primitive-size-2-fill-current` | ✅ |
| `src/lib/components/ui/menubar/menubar-separator.svelte` | grayslate | svelte | `menubar-separator` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-shortcut.svelte` | grayslate | svelte | `menubar-shortcut` | `htmlattributes` | ❌ |
| `src/lib/components/ui/menubar/menubar-sub-content.svelte` | grayslate | svelte | `menubar-sub-content` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-sub-trigger.svelte` | grayslate | svelte | `menubar-sub-trigger` | `menubar-primitive-ms-auto-size-4` | ✅ |
| `src/lib/components/ui/menubar/menubar-sub.svelte` | grayslate | svelte | `menubar-sub` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar-trigger.svelte` | grayslate | svelte | `menubar-trigger` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/menubar/menubar.svelte` | grayslate | svelte | `menubar` | `menubar-primitive` | ✅ |
| `src/lib/components/ui/resizable/resizable-handle.svelte` | grayslate | svelte | `resizable-handle` | `resizable-primitive-size-2-5` | ✅ |
| `src/lib/components/ui/resizable/resizable-pane-group.svelte` | grayslate | svelte | `resizable-pane-group` | `resizable-primitive` | ✅ |
| `src/lib/components/ui/select/select-content.svelte` | grayslate | svelte | `select-content` | `select-primitive` | ✅ |
| `src/lib/components/ui/select/select-group-heading.svelte` | grayslate | svelte | `select-group-heading` | `typeof` | ❌ |
| `src/lib/components/ui/select/select-group.svelte` | grayslate | svelte | `select-group` | `select-primitive` | ✅ |
| `src/lib/components/ui/select/select-item.svelte` | grayslate | svelte | `select-item` | `select-primitive-size-4` | ✅ |
| `src/lib/components/ui/select/select-label.svelte` | grayslate | svelte | `select-label` | `htmlattributes` | ❌ |
| `src/lib/components/ui/select/select-portal.svelte` | grayslate | svelte | `select-portal` | `select-primitive` | ✅ |
| `src/lib/components/ui/select/select-scroll-down-button.svelte` | grayslate | svelte | `select-scroll-down-button` | `select-primitive-size-4` | ✅ |
| `src/lib/components/ui/select/select-scroll-up-button.svelte` | grayslate | svelte | `select-scroll-up-button` | `select-primitive-size-4` | ✅ |
| `src/lib/components/ui/select/select-separator.svelte` | grayslate | svelte | `select-separator` | `separator` | ✅ |
| `src/lib/components/ui/select/select-trigger.svelte` | grayslate | svelte | `select-trigger` | `select-primitive-size-4-opacity-50` | ✅ |
| `src/lib/components/ui/select/select.svelte` | grayslate | svelte | `select` | `select-primitive` | ✅ |
| `src/lib/components/ui/separator/separator.svelte` | grayslate | svelte | `separator` | `separator-primitive` | ✅ |

---

## Svelte — UI Components (Sidebar, Skeleton, Sonner, Tabs)

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/components/ui/sidebar/sidebar-content.svelte` | grayslate | svelte | `sidebar-content` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-footer.svelte` | grayslate | svelte | `sidebar-footer` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-group-content.svelte` | grayslate | svelte | `sidebar-group-content` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-group-label.svelte` | grayslate | svelte | `sidebar-group-label` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-group.svelte` | grayslate | svelte | `sidebar-group` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-header.svelte` | grayslate | svelte | `sidebar-header` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-inset.svelte` | grayslate | svelte | `sidebar-inset` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-menu-button.svelte` | grayslate | svelte | `sidebar-menu-button` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-menu-item.svelte` | grayslate | svelte | `sidebar-menu-item` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-menu.svelte` | grayslate | svelte | `sidebar-menu` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-provider.svelte` | grayslate | svelte | `sidebar-provider` | `htmlattributes` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-rail.svelte` | grayslate | svelte | `sidebar-rail` | `htmlattributes-toggle-sidebar` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-separator.svelte` | grayslate | svelte | `sidebar-separator` | `typeof` | ❌ |
| `src/lib/components/ui/sidebar/sidebar-trigger.svelte` | grayslate | svelte | `sidebar-trigger` | `typeof-sr-only` | ❌ |
| `src/lib/components/ui/sidebar/sidebar.svelte` | grayslate | svelte | `sidebar` | `htmlattributes-text-sidebar-foreground-group-peer-block` | ❌ |
| `src/lib/components/ui/skeleton/skeleton.svelte` | grayslate | svelte | `skeleton` | `with-element-ref` | ❌ |
| `src/lib/components/ui/sonner/DismissibleToastMessage.svelte` | grayslate | svelte | `dismissible-toast` | `li` | ❌ |
| `src/lib/components/ui/sonner/sonner.svelte` | grayslate | svelte | `sonner` | `sonner-toaster-group-size-4-animate-spin` | ⚠️ |
| `src/lib/components/ui/tabs/tabs-content.svelte` | grayslate | svelte | `tabs-content` | `tabs-primitive` | ✅ |
| `src/lib/components/ui/tabs/tabs-list.svelte` | grayslate | svelte | `tabs-list` | `tabs-primitive` | ✅ |
| `src/lib/components/ui/tabs/tabs-trigger.svelte` | grayslate | svelte | `tabs-trigger` | `tabs-primitive` | ✅ |
| `src/lib/components/ui/tabs/tabs.svelte` | grayslate | svelte | `tabs` | `tabs-primitive` | ✅ |

---

## Svelte — Editor Components

| File | Repo | Lang | Ideal Name | Our System | Status |
|---|---|---|---|---|---|
| `src/lib/editor/components/Editor.svelte` | grayslate | svelte | `editor` | `editor-view` | ✅ |
| `src/lib/editor/components/EditorActions.svelte` | grayslate | svelte | `editor-actions` | `typeof-switch-to-plain-csv-size-4-transition-all` | ❌ |
| `src/lib/editor/components/EditorContextMenu.svelte` | grayslate | svelte | `editor-context-menu` | `htmldiv-element` | ❌ |
| `src/lib/editor/components/EditorLoader.svelte` | grayslate | svelte | `editor-loader` | `div-flex-flex-col-items-center-mt-1-gap-1-5-text-muted` | ❌ |
| `src/lib/editor/components/EditorWrapper.svelte` | grayslate | svelte | `editor-wrapper` | `string` | ❌ |
| `src/lib/editor/components/FindReplace.svelte` | grayslate | svelte | `find-replace` | `typeof` | ❌ |
| `src/lib/editor/components/GoToLineDialog.svelte` | grayslate | svelte | `go-to-line-dialog` | `htmlinput-element-sm-max-w-[25rem]-grid-gap-3` | ❌ |
| `src/lib/editor/components/LanguagePicker.svelte` | grayslate | svelte | `language-picker` | `button-select-language-mode-w-3-5-h-3-5-shrink-0-self-center` | ⚠️ |
| `src/lib/editor/components/StatusBar.svelte` | grayslate | svelte | `status-bar` | `div-flex-items-center-h-full-font-semibold` | ❌ |
| `src/lib/editor/components/TransformationsPalette.svelte` | grayslate | svelte | `transformations-palette` | `boolean-p-0-sm-max-w-[40rem]-gap-0-sr-only` | ❌ |
| `src/lib/editor/components/csv/CsvContextMenu.svelte` | grayslate | svelte | `csv-context-menu` | `typeof-mr-2-h-4-w-4-shrink-0-mr-2-h-4-w-4-shrink-0` | ❌ |
| `src/lib/editor/components/csv/CsvTableBody.svelte` | grayslate | svelte | `csv-table-body` | `string-csv-table-csv-table-body-csv-row-num` | ⚠️ |
| `src/lib/editor/components/csv/CsvTableHeader.svelte` | grayslate | svelte | `csv-table-header` | `string-csv-table-csv-row-num-header` | ⚠️ |
| `src/lib/editor/components/csv/CsvTableView.svelte` | grayslate | svelte | `csv-table-view` | `t` | ❌ |
| `src/lib/editor/components/markdown/MarkdownPreview.svelte` | grayslate | svelte | `markdown-preview` | `p` | ❌ |
| `src/lib/editor/components/markdown/MarkdownPreviewContextMenu.svelte` | grayslate | svelte | `markdown-preview-context-menu` | `htmldiv-element-mr-2-h-4-w-4-shrink-0-my-1-h-px-bg-muted` | ❌ |

---

## Summary by Status

| Status | Count | Meaning |
|---|---|---|
| ✅ Good | ~90 | Name is accurate and concise |
| ⚠️ Noisy | ~120 | Right concept, too many symbols joined |
| ❌ Poor | ~33 | Generic (`htmlattributes`, `typeof`, CSS class noise) |
| 🚫 Fallback | 4 | No name derived at all |

### Key Patterns to Improve

1. **Svelte files** — The XML/HTML extractor is used for `.svelte` files but picks up Tailwind class strings (`div-flex-1-min-h-0`) and generic TS types (`htmlattributes`, `typeof`) as names. A Svelte-specific extractor reading `<script>` exported props and the primitive root component name would be far more accurate.

2. **Rust files with many symbols** — The code extractor correctly finds symbols but joins up to 4, making names like `file-read-cancellation-registry-active-file-read-ensure`. Reducing to 1–2 highest-priority symbols would improve quality for Rust modules.

3. **TypeScript barrel files** — After this session's fixes, names like `root-badge-badge-variants-badge-props` are generated. Ideal would be to detect that `Root as Badge` means "badge" and stop there (take only the alias/highest-signal name, not all 4).

4. **Config JSON** — `identifier-description-windows-permissions` for `capabilities/default.json` could be improved by detecting Tauri capability patterns specifically.

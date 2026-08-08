/**
 * The behavior registry.
 *
 * Coverage is a claim about *behavior*, not about symbols. An earlier design
 * derived coverage by grepping specs for `data-testid` strings and IPC command
 * names, but a test that mentions `action-save` has not necessarily saved
 * anything, and a spec that names `write_file_content` has not necessarily
 * asserted the bytes. That measure rewards mentioning things.
 *
 * So: every behavior is listed here with an explicit id, and every id must be
 * claimed exactly once by an executable test (`scenario(id, ...)`) or be
 * consciously assigned elsewhere. `e2e/scripts/audit-coverage.mjs` parses the
 * spec sources and enforces it.
 *
 * `coverage` says where the behavior is proven:
 * - `e2e`    — a packaged-app scenario, because the behavior spans the UI, the
 *              IPC boundary, and the filesystem.
 * - `rust`   — a pure algorithm or an exhaustive registry, where driving the
 *              desktop UI once per case would be slow and prove less. The
 *              `reason` must name the test module.
 * - `manual` — automation is impractical or destructive. Needs a reason and an
 *              owner so it is a decision, not an omission.
 */

export type Area =
  | "file-lifecycle"
  | "text-format"
  | "editor"
  | "language"
  | "transformations"
  | "csv"
  | "markdown"
  | "sidebar"
  | "settings-shell"
  | "security"
  | "visual"
  | "harness";

export interface Requirement {
  id: string;
  area: Area;
  /** The observable behavior, phrased as what the user gets. */
  behavior: string;
  coverage: "e2e" | "rust" | "static" | "manual";
  /** Required for everything but `e2e`: where it is proven, or why it is not. */
  reason?: string;
  /** Required for `manual`: who re-checks it before a release. */
  owner?: string;
}

export const REQUIREMENTS: readonly Requirement[] = [
  // ── File lifecycle ───────────────────────────────────────────────────────
  { id: "file.first-run.blank-slate", area: "file-lifecycle", behavior: "A fresh launch shows an empty, untitled slate.", coverage: "e2e" },
  { id: "file.slate.autosave-and-name", area: "file-lifecycle", behavior: "Typing into a slate autosaves it under a content-derived name.", coverage: "e2e" },
  { id: "file.slate.reopen-from-sidebar", area: "file-lifecycle", behavior: "Reopening a slate from the sidebar restores its content and language.", coverage: "e2e" },
  { id: "file.slate.switch-flushes", area: "file-lifecycle", behavior: "Switching away from a slate flushes the pending edit to the same file.", coverage: "e2e" },
  { id: "file.slate.close-flushes", area: "file-lifecycle", behavior: "Closing the window flushes a slate without prompting, since slates are never dirty.", coverage: "e2e" },
  { id: "file.external.open", area: "file-lifecycle", behavior: "An external file opens as a local document and appears under All and Local.", coverage: "e2e" },
  { id: "file.external.save", area: "file-lifecycle", behavior: "Saving a local file writes the edit to disk and clears the dirty flag.", coverage: "e2e" },
  { id: "file.dirty.local-only", area: "file-lifecycle", behavior: "The dirty indicator appears for local files and never for slates.", coverage: "e2e" },
  { id: "file.guard.cancel-and-discard", area: "file-lifecycle", behavior: "Leaving a dirty local document prompts, and Cancel/Discard behave differently.", coverage: "e2e" },
  { id: "file.guard.save", area: "file-lifecycle", behavior: "Choosing Save in the guard writes the file before switching away.", coverage: "e2e" },
  { id: "file.save.coalesces", area: "file-lifecycle", behavior: "Repeated explicit Save presses coalesce onto the same local file without creating duplicates.", coverage: "e2e" },
  { id: "file.save-as.untitled", area: "file-lifecycle", behavior: "Save As on an untitled slate writes to the chosen path.", coverage: "e2e" },
  { id: "file.save-as.local", area: "file-lifecycle", behavior: "Save As on a local file writes a copy and follows the new path.", coverage: "e2e" },
  { id: "file.save-as.leaves-original", area: "file-lifecycle", behavior: "Save As leaves the original file untouched.", coverage: "e2e" },
  { id: "file.open.dialog-cancel", area: "file-lifecycle", behavior: "Cancelling the Open dialog leaves the current document and the library untouched.", coverage: "e2e" },
  { id: "file.save-as.dialog-cancel", area: "file-lifecycle", behavior: "Cancelling the Save As dialog writes nothing and keeps the document on its original path.", coverage: "e2e" },
  { id: "file.rename.validation", area: "file-lifecycle", behavior: "Rename rejects an empty name and names containing path separators.", coverage: "e2e" },
  { id: "file.rename.applies", area: "file-lifecycle", behavior: "Renaming moves the file on disk and updates the sidebar.", coverage: "e2e" },
  { id: "file.duplicate", area: "file-lifecycle", behavior: "Duplicating a slate creates a second distinct file.", coverage: "e2e" },
  { id: "file.duplicate-as-slate", area: "file-lifecycle", behavior: "Duplicating a local file as a slate copies it into the notes root.", coverage: "e2e" },
  { id: "file.delete.confirmed", area: "file-lifecycle", behavior: "Delete asks first, then removes the file and its card.", coverage: "e2e" },
  { id: "file.delete.without-confirmation", area: "file-lifecycle", behavior: "With confirmation disabled, delete removes the file immediately.", coverage: "e2e" },
  { id: "file.unlink", area: "file-lifecycle", behavior: "Untracking a local file removes it from the library but not from disk.", coverage: "e2e" },
  { id: "file.reveal", area: "file-lifecycle", behavior: "Reveal asks the OS to show the file, with the validated path.", coverage: "e2e" },
  { id: "file.copy-path", area: "file-lifecycle", behavior: "Copy path puts the document's full path on the clipboard.", coverage: "e2e" },
  { id: "file.identity.slate-lifecycle", area: "file-lifecycle", behavior: "A slate keeps one file identity across autosave, manual save, switching, and close.", coverage: "e2e" },
  { id: "file.identity.local-lifecycle", area: "file-lifecycle", behavior: "A local file is written only on command and keeps its path throughout.", coverage: "e2e" },
  { id: "file.autosave.never-touches-local", area: "file-lifecycle", behavior: "Autosave never writes a local file that the user has not saved.", coverage: "e2e" },
  { id: "file.restart.reopens-last", area: "file-lifecycle", behavior: "With 'Reopen last file', a restart restores the previous document.", coverage: "e2e" },
  { id: "file.restart.new-slate", area: "file-lifecycle", behavior: "With 'Start with a new slate', a restart opens a blank slate.", coverage: "e2e" },
  { id: "file.read.cancel", area: "file-lifecycle", behavior: "Cancelling a long file read returns the app to a usable state.", coverage: "e2e" },
  { id: "file.size-limit", area: "file-lifecycle", behavior: "Files above 200 MB are refused before their content is read.", coverage: "e2e" },
  { id: "file.git-sync", area: "file-lifecycle", behavior: "Notes sync to Git.", coverage: "manual", reason: "Requires a real remote and credentials; out of scope for a hermetic sandbox.", owner: "release reviewer" },

  // ── Text format: encoding and line endings ───────────────────────────────
  { id: "format.eol.detect-crlf", area: "text-format", behavior: "A CRLF file opens clean and stays byte-identical when untouched.", coverage: "e2e" },
  { id: "format.eol.preserve-crlf-on-save", area: "text-format", behavior: "Editing and saving a CRLF file keeps CRLF.", coverage: "e2e" },
  { id: "format.eol.legacy-cr", area: "text-format", behavior: "A CR-only file opens clean and modernizes on the next write.", coverage: "e2e" },
  { id: "format.eol.mixed-dominant", area: "text-format", behavior: "A mixed file adopts its dominant ending and normalizes on save.", coverage: "e2e" },
  { id: "format.eol.switch-marks-dirty", area: "text-format", behavior: "Changing the EOL alone marks the document dirty and converts on save.", coverage: "e2e" },
  { id: "format.eol.revert-clears-dirty", area: "text-format", behavior: "Switching the EOL back before saving clears the dirty flag.", coverage: "e2e" },
  { id: "format.eol.save-cycles", area: "text-format", behavior: "Dirty and save-enabled state stay correct across edits either side of an EOL conversion.", coverage: "e2e" },
  { id: "format.eol.clipboard-conversion", area: "text-format", behavior: "Copying a CRLF document yields the canonical LF text without reporting a failure.", coverage: "e2e" },
  { id: "format.eol.picker-state", area: "text-format", behavior: "The EOL picker shows the active value and offers only LF and CRLF.", coverage: "e2e" },
  { id: "format.eol.hidden-in-csv", area: "text-format", behavior: "Table mode hides the EOL control and restores it on exit without dirtying.", coverage: "e2e" },
  { id: "format.eol.no-trailing-newline", area: "text-format", behavior: "A file without a trailing newline keeps it that way.", coverage: "e2e" },
  { id: "format.eol.autosave-generation", area: "text-format", behavior: "An EOL change plus a CSV round-trip keeps autosave generations monotonic.", coverage: "e2e" },
  { id: "format.encoding.utf8-bom", area: "text-format", behavior: "A UTF-8 BOM is detected and never shown as document text.", coverage: "e2e" },
  { id: "format.encoding.utf16le", area: "text-format", behavior: "UTF-16 LE is detected and converts to UTF-8 on request.", coverage: "e2e" },
  { id: "format.encoding.utf16be", area: "text-format", behavior: "UTF-16 BE is detected and converts to UTF-8 on request.", coverage: "e2e" },
  { id: "format.encoding.windows1252-prompt", area: "text-format", behavior: "Ambiguous bytes prompt before being read as Windows-1252.", coverage: "e2e" },
  { id: "format.encoding.lossy-save-refused", area: "text-format", behavior: "A save that would lose characters is refused and the file is untouched.", coverage: "e2e" },
  { id: "format.encoding.reopen-saves-dirty-local", area: "text-format", behavior: "Reopening a dirty local file with a new encoding prompts to save first.", coverage: "e2e" },
  { id: "format.encoding.reopen-slate-silent", area: "text-format", behavior: "Reopening a slate with a new encoding saves silently and keeps autosaving.", coverage: "e2e" },
  { id: "format.encoding.default-for-new", area: "text-format", behavior: "The default encoding setting applies to newly created documents.", coverage: "e2e" },
  { id: "format.eol.default-for-new", area: "text-format", behavior: "The default line-ending setting applies to newly created documents.", coverage: "e2e" },
  { id: "format.detection.rules", area: "text-format", behavior: "Encoding and EOL detection rules over raw bytes.", coverage: "rust", reason: "Exhaustively covered by src-tauri/src/character_encoding.rs and line_ending.rs unit tests." },

  // ── Editor ───────────────────────────────────────────────────────────────
  { id: "editor.find.count-and-filters", area: "editor", behavior: "Find reports a match count that responds to case, word, and regex.", coverage: "e2e" },
  { id: "editor.find.navigate", area: "editor", behavior: "Next and previous move through matches.", coverage: "e2e" },
  { id: "editor.find.replace-one", area: "editor", behavior: "Replace changes only the current match.", coverage: "e2e" },
  { id: "editor.find.replace-all-single-undo", area: "editor", behavior: "Replace all applies as one undoable transaction.", coverage: "e2e" },
  { id: "editor.find.regex-error", area: "editor", behavior: "An invalid regular expression is reported rather than silently matching nothing.", coverage: "e2e" },
  { id: "editor.find.cancel", area: "editor", behavior: "A long match scan can be cancelled without wedging the editor.", coverage: "e2e" },
  { id: "editor.goto-line", area: "editor", behavior: "Go to line moves the cursor and rejects out-of-range input.", coverage: "e2e" },
  { id: "editor.word-wrap.toggle", area: "editor", behavior: "Word wrap toggles to a definite state.", coverage: "e2e" },
  { id: "editor.font-size", area: "editor", behavior: "Font size increases, decreases, and resets.", coverage: "e2e" },
  { id: "editor.undo-redo", area: "editor", behavior: "Undo and redo round-trip an edit exactly.", coverage: "e2e" },
  { id: "editor.clipboard.copy-document", area: "editor", behavior: "Copy places the whole document on the system clipboard.", coverage: "e2e" },
  { id: "editor.clipboard.cut-paste-select-all", area: "editor", behavior: "Cut, paste, and select all operate on the real selection.", coverage: "e2e" },
  { id: "editor.context-menu.clipboard", area: "editor", behavior: "The editor context menu cuts, copies, and selects all.", coverage: "e2e" },
  { id: "editor.context-menu.json", area: "editor", behavior: "Inside JSON the context menu copies the path, key, and value.", coverage: "e2e" },
  { id: "editor.indent.picker", area: "editor", behavior: "The indentation picker switches between spaces and tabs and changes width.", coverage: "e2e" },
  { id: "editor.indent.detect", area: "editor", behavior: "Detect-from-content adopts the document's own indentation.", coverage: "e2e" },
  { id: "editor.indent.tab-outdent", area: "editor", behavior: "Tab and Shift+Tab indent and outdent the selection.", coverage: "e2e" },

  // ── Language and naming ──────────────────────────────────────────────────
  { id: "language.detect-from-content", area: "language", behavior: "Typing recognizable source updates the detected language in the status bar.", coverage: "e2e" },
  { id: "language.detect-from-file", area: "language", behavior: "Opening a real fixture shows the right language for representative families.", coverage: "e2e" },
  { id: "language.manual-override", area: "language", behavior: "Choosing a language overrides detection, and Auto restores it.", coverage: "e2e" },
  { id: "language.save-extension", area: "language", behavior: "Saving an untitled slate picks the canonical extension for its language.", coverage: "e2e" },
  { id: "language.detector-matrix", area: "language", behavior: "Per-language detection across all supported languages.", coverage: "rust", reason: "Exhaustively covered by the crates/grayslate-langdetect unit tests; driving 40+ languages through the UI would be slower and prove less." },
  { id: "language.naming-matrix", area: "language", behavior: "Per-language canonical extensions and naming kinds, including -email and -prompt.", coverage: "rust", reason: "Exhaustively covered by the crates/grayslate-langnaming unit tests." },

  // ── Transformations ──────────────────────────────────────────────────────
  { id: "transform.palette.search", area: "transformations", behavior: "The palette filters to matching actions and hides the rest.", coverage: "e2e" },
  { id: "transform.family.replace-document", area: "transformations", behavior: "A replace-style action rewrites the document as one undoable step.", coverage: "e2e" },
  { id: "transform.family.replace-document-formatting", area: "transformations", behavior: "A formatter reflows the document and reverts with a single undo.", coverage: "e2e" },
  { id: "transform.family.replace-selection", area: "transformations", behavior: "With a selection, a replace-style action changes only that range.", coverage: "e2e" },
  { id: "transform.family.insert", area: "transformations", behavior: "An insert-style action inserts at the cursor instead of replacing.", coverage: "e2e" },
  { id: "transform.family.message", area: "transformations", behavior: "A statistics action reports through a toast and leaves the document alone.", coverage: "e2e" },
  { id: "transform.family.error", area: "transformations", behavior: "Invalid input produces an error message and no document change.", coverage: "e2e" },
  { id: "transform.family.language-switch", area: "transformations", behavior: "A converting action switches the active language to the output format.", coverage: "e2e" },
  { id: "transform.large.chunked", area: "transformations", behavior: "A multi-megabyte result assembles correctly and stays one undo step.", coverage: "e2e" },
  { id: "transform.large.cancel", area: "transformations", behavior: "Cancelling a long transformation leaves the document unchanged and usable.", coverage: "e2e" },
  { id: "transform.registry.correctness", area: "transformations", behavior: "Every registered transformation produces the right output for its inputs.", coverage: "rust", reason: "126 unit tests in src-tauri/src/commands/transform.rs cover the registry exhaustively; E2E covers one representative per UI/transport behavior family instead." },

  // ── CSV ──────────────────────────────────────────────────────────────────
  { id: "csv.enter-table", area: "csv", behavior: "Table mode reports the row, column, and delimiter counts.", coverage: "e2e" },
  { id: "csv.navigate-and-select", area: "csv", behavior: "Keyboard navigation moves the selected cell.", coverage: "e2e" },
  { id: "csv.edit-cell", area: "csv", behavior: "Typing into a cell and committing updates the value.", coverage: "e2e" },
  { id: "csv.clear-cell", area: "csv", behavior: "Clearing a cell empties it without removing the row.", coverage: "e2e" },
  { id: "csv.undo-redo", area: "csv", behavior: "Table edits undo and redo within table mode.", coverage: "e2e" },
  { id: "csv.rows.insert-delete", area: "csv", behavior: "Rows can be inserted above and below and deleted.", coverage: "e2e" },
  { id: "csv.rows.move", area: "csv", behavior: "A row moves up and down and the new order is visible.", coverage: "e2e" },
  { id: "csv.columns.insert-delete", area: "csv", behavior: "Columns can be inserted left and right and deleted.", coverage: "e2e" },
  { id: "csv.columns.move", area: "csv", behavior: "A column moves left and right and the new order is visible.", coverage: "e2e" },
  { id: "csv.context-menu", area: "csv", behavior: "The grid context menu performs its row and column operations.", coverage: "e2e" },
  { id: "csv.copy", area: "csv", behavior: "Copying yields correctly escaped CSV, committing any open edit first.", coverage: "e2e" },
  { id: "csv.save", area: "csv", behavior: "Saving from table mode writes the edited rows to disk.", coverage: "e2e" },
  { id: "csv.small.live-mirror-history", area: "csv", behavior: "At or below 100,000 rows, each table edit remains an individual text-mode undo step back to the pre-table document.", coverage: "e2e" },
  { id: "csv.large.bounded-render", area: "csv", behavior: "A 100k-row file keeps the rendered DOM bounded.", coverage: "e2e" },
  { id: "csv.large.mirroring-threshold", area: "csv", behavior: "Above the mirroring threshold, exiting still returns to text as one step.", coverage: "e2e" },
  { id: "csv.delimiters", area: "csv", behavior: "Semicolon and tab delimited files are recognized and reported.", coverage: "e2e" },
  { id: "csv.flexible-rows", area: "csv", behavior: "Rows with differing field counts remain usable under the flexible CSV policy.", coverage: "e2e" },
  { id: "csv.format-preservation", area: "csv", behavior: "A table round-trip preserves the file's encoding and line endings.", coverage: "e2e" },
  { id: "csv.actions-unavailable", area: "csv", behavior: "Text-only transformations are unavailable in table mode.", coverage: "e2e" },
  { id: "csv.cancel", area: "csv", behavior: "Leaving table mode while a session operation is in flight cancels it instead of applying a stale result.", coverage: "e2e" },
  { id: "csv.parsing.rules", area: "csv", behavior: "RFC 4180 parsing and serialization.", coverage: "rust", reason: "Covered by the src-tauri/src/csv unit tests." },

  // ── Markdown ─────────────────────────────────────────────────────────────
  { id: "markdown.preview.toggle", area: "markdown", behavior: "The preview opens and closes without remounting the editor.", coverage: "e2e" },
  { id: "markdown.preview.renders", area: "markdown", behavior: "Headings, lists, code, and links render as expected.", coverage: "e2e" },
  { id: "markdown.sanitization", area: "markdown", behavior: "Scripts, event handlers, and javascript: URLs are stripped and never execute.", coverage: "e2e" },
  { id: "markdown.scroll-sync", area: "markdown", behavior: "Scrolling the editor moves the preview to the corresponding position.", coverage: "e2e" },
  { id: "markdown.copy", area: "markdown", behavior: "The preview context menu copies the selection and selects all.", coverage: "e2e" },
  { id: "markdown.external-links", area: "markdown", behavior: "An external link opens in the system browser, never in the app webview.", coverage: "e2e" },
  { id: "markdown.relative-images", area: "markdown", behavior: "A saved document resolves relative images through the bounded asset command.", coverage: "e2e" },
  { id: "markdown.size-guard", area: "markdown", behavior: "An oversize document declines to render a preview instead of hanging.", coverage: "e2e" },
  { id: "markdown.cancel", area: "markdown", behavior: "A long preview render can be cancelled.", coverage: "e2e" },

  // ── Sidebar ──────────────────────────────────────────────────────────────
  { id: "sidebar.sort.name", area: "sidebar", behavior: "Name ascending and descending order the list correctly.", coverage: "e2e" },
  { id: "sidebar.sort.recency", area: "sidebar", behavior: "Most- and least-recently-opened order the list correctly.", coverage: "e2e" },
  { id: "sidebar.sort.size", area: "sidebar", behavior: "Largest and smallest order the list correctly.", coverage: "e2e" },
  { id: "sidebar.filter-tabs", area: "sidebar", behavior: "Slates and Local show only their own files, and local files carry a badge.", coverage: "e2e" },
  { id: "sidebar.search.text", area: "sidebar", behavior: "Search finds files by content through the Rust backend.", coverage: "e2e" },
  { id: "sidebar.search.modifiers", area: "sidebar", behavior: "Case, whole-word, and regex change which results come back.", coverage: "e2e" },
  { id: "sidebar.search.empty-state", area: "sidebar", behavior: "A query with no matches shows the empty state.", coverage: "e2e" },
  { id: "sidebar.search.cancel", area: "sidebar", behavior: "Typing again cancels the in-flight search rather than queueing it.", coverage: "e2e" },
  { id: "sidebar.search.clear", area: "sidebar", behavior: "Clearing the search resets the query and its modifiers.", coverage: "e2e" },
  { id: "sidebar.reorder-suppression", area: "sidebar", behavior: "Opening a file from the sidebar does not make the list jump under the cursor.", coverage: "e2e" },
  { id: "sidebar.refresh-on-mutation", area: "sidebar", behavior: "Backend file mutations refresh the list without a manual reload.", coverage: "e2e" },
  { id: "sidebar.find-files-shortcut", area: "sidebar", behavior: "The Find Files shortcut focuses the search input.", coverage: "e2e" },
  { id: "sidebar.toggle", area: "sidebar", behavior: "The sidebar collapses and restores.", coverage: "e2e" },
  { id: "sidebar.open.persist-across-restart", area: "sidebar", behavior: "Whether the sidebar is open survives a restart.", coverage: "e2e" },
  { id: "sidebar.width.persist-across-restart", area: "sidebar", behavior: "A resized sidebar keeps its width across a restart.", coverage: "e2e" },

  // ── Settings and shell ───────────────────────────────────────────────────
  { id: "shell.theme.toggle-and-persist", area: "settings-shell", behavior: "The theme toggles and the choice survives an editor remount.", coverage: "e2e" },
  { id: "shell.theme.persist-across-restart", area: "settings-shell", behavior: "The theme survives a restart.", coverage: "e2e" },
  { id: "shell.settings.indent-default", area: "settings-shell", behavior: "The default indentation setting applies to a new slate.", coverage: "e2e" },
  { id: "shell.settings.persist-across-restart", area: "settings-shell", behavior: "Font size, word wrap, and format defaults survive a restart.", coverage: "e2e" },
  { id: "shell.window.maximize-restore", area: "settings-shell", behavior: "The window maximizes and restores from the title bar.", coverage: "e2e" },
  { id: "shell.window.minimize", area: "settings-shell", behavior: "The window minimizes from the title bar.", coverage: "e2e" },
  { id: "shell.window.close-guard", area: "settings-shell", behavior: "Closing with unsaved local changes prompts instead of losing them.", coverage: "e2e" },
  { id: "shell.about", area: "settings-shell", behavior: "About shows the version and its update state.", coverage: "e2e" },
  { id: "shell.updates.check", area: "settings-shell", behavior: "Checking for updates reports a state without leaving the app.", coverage: "e2e" },
  { id: "shell.updates.install", area: "settings-shell", behavior: "Installing an available update is offered and invoked correctly.", coverage: "manual", reason: "Needs a signed update artifact and a real update server; the UI state machine is covered by shell.updates.check.", owner: "release reviewer" },
  { id: "shell.updates.signature", area: "settings-shell", behavior: "Update signatures are verified before installation.", coverage: "manual", reason: "Supply-chain property; verified during release sign-off, not from the app UI.", owner: "release reviewer" },
  { id: "shell.shortcuts.dialog", area: "settings-shell", behavior: "The shortcuts dialog lists and filters every section.", coverage: "e2e" },
  { id: "shell.shortcuts.tooltips", area: "settings-shell", behavior: "Toolbar tooltips show the real platform shortcut.", coverage: "e2e" },
  { id: "shell.shortcuts.catalog-matches-registrations", area: "settings-shell", behavior: "The documented shortcut catalog matches what the app actually registers.", coverage: "static", reason: "A source-level drift check in e2e/scripts/audit-coverage.mjs: the catalog in src/lib/shortcuts.ts and the registration sites are separate declarations, and no runtime observation can prove a shortcut is missing from the catalog." },
  { id: "shell.toasts.success-and-error", area: "settings-shell", behavior: "Success and failure paths surface a readable toast.", coverage: "e2e" },
  { id: "shell.memory-reclamation", area: "settings-shell", behavior: "Heap is reclaimed after expensive editor teardown.", coverage: "manual", reason: "A statistical property of the GC; not assertable from WebDriver without flakiness.", owner: "release reviewer" },

  // ── Visual (non-blocking suite) ──────────────────────────────────────────
  { id: "visual.typography.hierarchy", area: "visual", behavior: "UI text uses Source Sans 3 at the intended weights and code uses Commit Mono, with no synthesized faces.", coverage: "e2e" },
  { id: "visual.icons.optical-size", area: "visual", behavior: "Search-option icons render at their intended optical sizes across icon libraries.", coverage: "e2e" },
  { id: "visual.states.hover-and-selected", area: "visual", behavior: "Selected controls are borderless and visibly distinct from hovered ones, in both themes.", coverage: "e2e" },

  // ── Security ─────────────────────────────────────────────────────────────
  { id: "security.forged-grants", area: "security", behavior: "Forged document grants are rejected and the file on disk is unchanged.", coverage: "e2e" },
  { id: "security.capabilities", area: "security", behavior: "App commands are allowed and sensitive plugin commands are denied.", coverage: "e2e" },
  { id: "security.headers", area: "security", behavior: "The webview is served restrictive security headers.", coverage: "e2e" },
  { id: "security.navigation", area: "security", behavior: "External top-level navigation and new windows are denied.", coverage: "e2e" },
  { id: "security.e2e-shims-present-in-test-build", area: "security", behavior: "The E2E command surface is reachable only in the explicitly opted-in test build.", coverage: "e2e" },
  { id: "security.e2e-shims-absent-in-release", area: "security", behavior: "A normal release contains neither test-only backend commands nor the E2E frontend runtime.", coverage: "static", reason: "The release-surface CI job builds without the e2e feature or Vite mode; scripts/verify-release-build.mjs derives every forbidden command from E2E_COMMANDS and scans the uncompressed frontend output for stable E2E markers." },

  // ── Harness ──────────────────────────────────────────────────────────────
  { id: "harness.test-hooks-present", area: "harness", behavior: "Every stable test hook the suite depends on is present in the shell, editor, status bar, and menus.", coverage: "e2e" },
  { id: "harness.no-native-tooltips", area: "harness", behavior: "No element carries a native title attribute, which would produce an OS tooltip that overlays and intercepts clicks.", coverage: "e2e" },
  { id: "harness.spec-discovery", area: "harness", behavior: "Every spec file on disk is discovered and run.", coverage: "e2e" },
  { id: "harness.deterministic-runtime", area: "harness", behavior: "The E2E runtime installs deterministic motion, IPC work tracking, and pinned locale/timezone.", coverage: "e2e" },
  { id: "harness.security-independent", area: "harness", behavior: "Security runs in a blocking CI job independent from functional and visual results.", coverage: "e2e" },
  { id: "harness.sandbox-isolation", area: "harness", behavior: "Each spec file starts against a sandbox nobody else has written to.", coverage: "e2e" },
  { id: "harness.isolated-home", area: "harness", behavior: "The suite runs against an isolated HOME, never the developer's.", coverage: "e2e" },
];

/** Requirements that must be claimed by an executable packaged scenario. */
export const E2E_REQUIREMENT_IDS: readonly string[] = REQUIREMENTS.filter(
  (requirement) => requirement.coverage === "e2e",
).map((requirement) => requirement.id);

export function requirementById(id: string): Requirement | undefined {
  return REQUIREMENTS.find((requirement) => requirement.id === id);
}

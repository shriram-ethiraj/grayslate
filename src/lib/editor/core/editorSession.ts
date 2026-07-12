import { Compartment, EditorState, Transaction, type Annotation } from "@codemirror/state";
import { indentLess, indentMore, isolateHistory, redo } from "@codemirror/commands";
import { indentUnit } from "@codemirror/language";
import { EditorView, gutters, keymap } from "@codemirror/view";
import { basicSetup } from "codemirror";
import { search } from "@codemirror/search";
import { scrollPastEnd } from "@codemirror/view";
import { createTheme } from "$lib/hooks/create-theme";
import { andromedaConfig } from "$lib/themes/andromeda";
import { materialLightConfig } from "$lib/themes/material-light";
import { colorHints } from "$lib/editor/extensions/colorHints";
import { getLanguageExtension } from "$lib/editor/config/languageExtensions";
import { contextMenuExtension } from "$lib/editor/extensions/contextMenuExtension";
import {
    editorState,
    openFindReplacePanel,
    openGoToLinePanel,
} from "$lib/state/editor.svelte";
import { getMinimalTextChange, type TextChangeSpec } from "$lib/editor/core/csvCodeMirror";
import type { IndentConfig } from "$lib/editor/components/IndentationPicker.svelte";

// ---------------------------------------------------------------------------
// Large-document value-sync debounce
// ---------------------------------------------------------------------------
// For documents above this threshold (in characters), `syncBindings` defers
// the expensive `doc.toString()` serialization by VALUE_SYNC_DEBOUNCE_MS.
// O(1) metadata (doc.length, doc.lines) is always pushed immediately.
const LARGE_DOC_THRESHOLD = 5_000_000; // ~10 MB UTF-16
const VALUE_SYNC_DEBOUNCE_MS = 300;

// ---------------------------------------------------------------------------
// Large-document scroll performance
// ---------------------------------------------------------------------------
// Documents above this line count have heavy viewport-driven decorations
// (colorHints, JSON inlay hints, fold widgets, etc.) disabled.  These
// extensions fire on every viewport shift and cause frame drops during fast
// scroll.  Syntax highlighting (Lezer) is NOT stripped — it runs
// incrementally and doesn't block the compositor.
const LARGE_DOC_LINE_THRESHOLD = 100_000;

const valueSyncTimers = new WeakMap<ManagedEditorSession, ReturnType<typeof setTimeout>>();

function clearValueSyncTimer(session: ManagedEditorSession): void {
    const timer = valueSyncTimers.get(session);
    if (timer !== undefined) {
        clearTimeout(timer);
        valueSyncTimers.delete(session);
    }
}

function openFindReplaceFromSelection(targetView: EditorView, replaceMode: boolean): boolean {
    editorState.activeView = targetView;
    return openFindReplacePanel(replaceMode);
}

type SessionBindings = {
    setValue: (value: string) => void;
    setDocumentLength: (length: number) => void;
    setLineCount: (count: number) => void;
    setLine: (line: number) => void;
    setCol: (col: number) => void;
    setSelectionSize: (size: number) => void;
    onViewUpdate: (view: EditorView, docChanged: boolean) => void;
};

export type ManagedEditorSession = {
    state?: EditorState;
    view?: EditorView;
    themeCompartment?: Compartment;
    fontSizeCompartment?: Compartment;
    langCompartment?: Compartment;
    wordWrapCompartment?: Compartment;
    decorationCompartment?: Compartment;
    indentCompartment?: Compartment;
    bindings?: SessionBindings;
};

export const DEFAULT_INDENT_CONFIG: IndentConfig = { indentMode: "spaces", indentSize: 2 };

// `indentUnit` controls what the Tab key inserts (see `insertIndentUnit`) and
// what `indentMore`/`indentLess` add/remove, and is also sent to the backend
// formatter. `EditorState.tabSize` controls the tab-stop DISPLAY WIDTH of any
// tab character in the document — in tab mode this is the size the picker
// exposes; in spaces mode it only affects stray tab chars, matching VSCode.
// Changing it re-flows how existing tabs render; it never changes document
// text (VSCode/Sublime/Notepad++ all behave this way).
function buildIndentExtension(config: IndentConfig) {
    const unit = config.indentMode === "tab" ? "\t" : " ".repeat(config.indentSize);
    return [indentUnit.of(unit), EditorState.tabSize.of(config.indentSize)];
}

function createFontSizeExtension(fontSize: number) {
    // An explicit line-height gives CodeMirror's height-map B-tree a
    // stable, predictable value for every line.  Without it, the browser
    // infers a height that can vary with font-loading and subpixel
    // rounding, causing viewport mis-estimations and scroll jumps.
    const lineHeight = `${Math.round(fontSize * 1.5)}px`;
    return EditorView.theme({
        // eslint-disable-next-line @typescript-eslint/naming-convention
        "&": {
            fontSize: `${fontSize}px`,
            lineHeight,
        },
        // eslint-disable-next-line @typescript-eslint/naming-convention
        ".cm-content": {
            lineHeight,
        },
    });
}

function syncBindings(
    session: ManagedEditorSession,
    state: EditorState,
    view?: EditorView,
    docChanged = true,
) {
    const bindings = session.bindings;
    if (!bindings) {
        return;
    }

    // Only update document metadata when the document actually changed.
    if (docChanged) {
        // O(1) metadata — always pushed immediately.
        bindings.setDocumentLength(state.doc.length);
        bindings.setLineCount(state.doc.lines);

        // Full-text serialization: doc.toString() is O(n) on the rope
        // and triggers an O(n) Svelte equality check. For documents above
        // LARGE_DOC_THRESHOLD we debounce the serialization so that rapid
        // keystrokes don't each pay the full O(n) cost.
        clearValueSyncTimer(session);

        if (state.doc.length < LARGE_DOC_THRESHOLD) {
            bindings.setValue(state.doc.toString());
        } else {
            const timer = setTimeout(() => {
                valueSyncTimers.delete(session);
                if (session.state && session.bindings) {
                    session.bindings.setValue(session.state.doc.toString());
                }
            }, VALUE_SYNC_DEBOUNCE_MS);
            valueSyncTimers.set(session, timer);
        }
    }

    const main = state.selection.main;
    const lineInfo = state.doc.lineAt(main.head);
    bindings.setLine(lineInfo.number);
    bindings.setCol(main.head - lineInfo.from + 1);
    bindings.setSelectionSize(
        state.selection.ranges.reduce((sum, range) => sum + (range.to - range.from), 0),
    );

    if (view) {
        bindings.onViewUpdate(view, docChanged);
    }
}

// VSCode inserts the configured indent unit (spaces by default) at the cursor
// on Tab when nothing is selected, only falling back to whole-line indent
// (indentMore) when a selection spans one or more lines.
function insertIndentUnit(view: EditorView) {
    if (view.state.selection.ranges.some((range) => !range.empty)) {
        return indentMore(view);
    }
    view.dispatch(
        view.state.update(view.state.replaceSelection(view.state.facet(indentUnit)), {
            scrollIntoView: true,
            userEvent: "input",
        }),
    );
    return true;
}

function createSearchKeymap() {
    return keymap.of([
        { key: "Tab", run: insertIndentUnit, shift: indentLess, preventDefault: true },
        { key: "Mod-y", run: redo, preventDefault: true },
        { key: "Mod-Shift-z", run: redo, preventDefault: true },
        {
            key: "Mod-g",
            run: () => openGoToLinePanel(),
            preventDefault: true,
        },
        {
            key: "Mod-f",
            run: (targetView) => openFindReplaceFromSelection(targetView, false),
            preventDefault: true,
        },
        {
            key: "Mod-h",
            run: (targetView) => openFindReplaceFromSelection(targetView, true),
            preventDefault: true,
        },
        {
            key: "Mod-Alt-f",
            run: (targetView) => openFindReplaceFromSelection(targetView, true),
            preventDefault: true,
        },
    ]);
}

export function createManagedEditorSession(): ManagedEditorSession {
    return {};
}

export function attachSessionBindings(
    session: ManagedEditorSession,
    bindings: SessionBindings,
) {
    session.bindings = bindings;
    if (session.state) {
        syncBindings(session, session.state, session.view);
    }
}

export function detachSessionBindings(session: ManagedEditorSession) {
    clearValueSyncTimer(session);
    session.bindings = undefined;
}

export function ensureManagedEditorState(
    session: ManagedEditorSession,
    doc: string,
    language: string,
): EditorState {
    if (session.state) {
        return session.state;
    }

    session.themeCompartment = new Compartment();
    session.fontSizeCompartment = new Compartment();
    session.langCompartment = new Compartment();
    session.wordWrapCompartment = new Compartment();
    session.decorationCompartment = new Compartment();
    session.indentCompartment = new Compartment();

    const isDark = document.documentElement.classList.contains("dark");
    const initialThemeExt = createTheme(
        isDark ? andromedaConfig : materialLightConfig,
    );

    const lineCount = doc.split("\n").length;
    const isLargeDoc = lineCount > LARGE_DOC_LINE_THRESHOLD;

    session.state = EditorState.create({
        doc,
        extensions: [
            createSearchKeymap(),
            gutters(),
            basicSetup,
            search({}),
            scrollPastEnd(),
            session.themeCompartment.of(initialThemeExt),
            session.fontSizeCompartment.of(createFontSizeExtension(editorState.fontSize)),
            session.langCompartment.of(getLanguageExtension(language)),
            session.wordWrapCompartment.of(
                editorState.wordWrap ? EditorView.lineWrapping : [],
            ),
            session.decorationCompartment.of(isLargeDoc ? [] : colorHints),
            session.indentCompartment.of(buildIndentExtension(DEFAULT_INDENT_CONFIG)),
            contextMenuExtension,
            EditorView.contentAttributes.of({ spellcheck: "false" }),
            EditorView.updateListener.of((update) => {
                session.state = update.state;
                if (update.selectionSet || update.docChanged) {
                    syncBindings(session, update.state, update.view, update.docChanged);
                }
            }),
        ],
    });

    syncBindings(session, session.state);
    return session.state;
}

export function setManagedEditorLanguage(
    session: ManagedEditorSession,
    language: string,
) {
    if (!session.view || !session.langCompartment) {
        return;
    }

    session.view.dispatch({
        effects: session.langCompartment.reconfigure(getLanguageExtension(language)),
    });
}

export function setManagedEditorFontSize(
    session: ManagedEditorSession,
    fontSize: number,
) {
    if (!session.view || !session.fontSizeCompartment) {
        return;
    }

    session.view.dispatch({
        effects: session.fontSizeCompartment.reconfigure(
            createFontSizeExtension(fontSize),
        ),
    });
}

export function setManagedEditorWordWrap(
    session: ManagedEditorSession,
    enabled: boolean,
) {
    if (!session.view || !session.wordWrapCompartment) {
        return;
    }

    const scrollTop = session.view.scrollDOM.scrollTop;
    const topPos = session.view.lineBlockAtHeight(scrollTop).from;

    session.view.dispatch({
        effects: session.wordWrapCompartment.reconfigure(
            enabled ? EditorView.lineWrapping : [],
        ),
    });

    session.view.requestMeasure({
        read(view) {
            return view.lineBlockAt(topPos).top;
        },
        write(newTop, view) {
            view.scrollDOM.scrollTop = newTop;
        },
    });
}

export function setManagedEditorIndent(
    session: ManagedEditorSession,
    config: IndentConfig,
) {
    if (!session.view || !session.indentCompartment) {
        return;
    }

    session.view.dispatch({
        effects: session.indentCompartment.reconfigure(buildIndentExtension(config)),
    });
}

export function captureManagedEditorView(
    session: ManagedEditorSession,
    view: EditorView,
) {
    session.state = view.state;
    session.view = undefined;
}

/**
 * Cancel any pending debounced value sync and immediately serialize
 * the current document into the bound `value`.
 *
 * Call this before save operations so that `value` and `isDirty`
 * reflect the freshest editor content regardless of debounce state.
 */
export function flushPendingValueSync(session: ManagedEditorSession): void {
    clearValueSyncTimer(session);
    if (session.state && session.bindings) {
        session.bindings.setValue(session.state.doc.toString());
    }
}

export function disposeManagedEditorSession(session: ManagedEditorSession) {
    clearValueSyncTimer(session);
    session.view = undefined;
    session.bindings = undefined;
    session.themeCompartment = undefined;
    session.fontSizeCompartment = undefined;
    session.langCompartment = undefined;
    session.wordWrapCompartment = undefined;
    session.indentCompartment = undefined;
    session.state = undefined;
}

export function dispatchManagedEditorChange(
    session: ManagedEditorSession,
    changes: TextChangeSpec,
    options?: {
        userEvent?: string;
        focus?: boolean;
        separateUndoStep?: boolean;
        addToHistory?: boolean;
        /**
         * Explicit post-change selection. Positions refer to the document
         * after `changes` is applied, matching CodeMirror's transaction API.
         */
        selection?: {
            anchor: number;
            head?: number;
        };
    },
): boolean {
    if (!session.state) {
        return false;
    }

    const annotations: Annotation<unknown>[] = [];
    if (options?.separateUndoStep) {
        annotations.push(isolateHistory.of("full"));
    }
    if (options?.addToHistory === false) {
        annotations.push(Transaction.addToHistory.of(false));
    }

    if (session.view) {
        session.view.dispatch({
            changes,
            selection: options?.selection,
            userEvent: options?.userEvent ?? "input",
            annotations: annotations.length > 0 ? annotations : undefined,
        });

        if (options?.focus !== false) {
            session.view.focus();
        }
        return true;
    }

    session.state = session.state.update({
        changes,
        selection: options?.selection,
        userEvent: options?.userEvent ?? "input",
        annotations: annotations.length > 0 ? annotations : undefined,
    }).state;
    syncBindings(session, session.state);
    return true;
}

export function dispatchManagedEditorTextChange(
    session: ManagedEditorSession,
    nextText: string,
    options?: {
        userEvent?: string;
        focus?: boolean;
        separateUndoStep?: boolean;
        addToHistory?: boolean;
    },
): boolean {
    if (!session.state) {
        return false;
    }

    const previousText = session.state.doc.toString();
    const changes = getMinimalTextChange(previousText, nextText);
    if (!changes) {
        return false;
    }

    return dispatchManagedEditorChange(session, changes, options);
}

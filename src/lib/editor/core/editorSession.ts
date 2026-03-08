import { Compartment, EditorState, Transaction, type Annotation } from "@codemirror/state";
import { isolateHistory, redo } from "@codemirror/commands";
import { EditorView, keymap } from "@codemirror/view";
import { basicSetup } from "codemirror";
import { search } from "@codemirror/search";
import { scrollPastEnd } from "@codemirror/view";
import { createTheme } from "$lib/hooks/create-theme";
import { andromedaConfig } from "$lib/themes/andromeda";
import { materialLightConfig } from "$lib/themes/material-light";
import { colorHints } from "$lib/editor/extensions/colorHints";
import { getLanguageExtension } from "$lib/editor/config/languageExtensions";
import { contextMenuExtension } from "$lib/editor/extensions/contextMenuExtension";
import { editorState } from "$lib/state/editor.svelte";
import { getMinimalTextChange, type TextChangeSpec } from "$lib/editor/core/csvCodeMirror";

type SessionBindings = {
    setValue: (value: string) => void;
    setLine: (line: number) => void;
    setCol: (col: number) => void;
    setSelectionSize: (size: number) => void;
    onViewUpdate: (view: EditorView) => void;
};

export type ManagedEditorSession = {
    state?: EditorState;
    view?: EditorView;
    themeCompartment?: Compartment;
    fontSizeCompartment?: Compartment;
    langCompartment?: Compartment;
    wordWrapCompartment?: Compartment;
    bindings?: SessionBindings;
};

function createFontSizeExtension(fontSize: number) {
    return EditorView.theme({
        // eslint-disable-next-line @typescript-eslint/naming-convention
        "&": {
            fontSize: `${fontSize}px`,
        },
    });
}

function syncBindings(
    session: ManagedEditorSession,
    state: EditorState,
    view?: EditorView,
) {
    const bindings = session.bindings;
    if (!bindings) {
        return;
    }

    const main = state.selection.main;
    const lineInfo = state.doc.lineAt(main.head);

    bindings.setValue(state.doc.toString());
    bindings.setLine(lineInfo.number);
    bindings.setCol(main.head - lineInfo.from + 1);
    bindings.setSelectionSize(
        state.selection.ranges.reduce((sum, range) => sum + (range.to - range.from), 0),
    );

    if (view) {
        bindings.onViewUpdate(view);
    }
}

function createSearchKeymap() {
    return keymap.of([
        { key: "Mod-y", run: redo, preventDefault: true },
        { key: "Mod-Shift-z", run: redo, preventDefault: true },
        {
            key: "Mod-f",
            run: (targetView) => {
                editorState.findReplace.visible = true;
                editorState.findReplace.replaceMode = false;
                const selection = targetView.state.selection.main;
                if (!selection.empty) {
                    editorState.findReplace.findText = targetView.state.sliceDoc(
                        selection.from,
                        selection.to,
                    );
                }
                return true;
            },
            preventDefault: true,
        },
        {
            key: "Mod-Alt-f",
            run: (targetView) => {
                editorState.findReplace.visible = true;
                editorState.findReplace.replaceMode = true;
                const selection = targetView.state.selection.main;
                if (!selection.empty) {
                    editorState.findReplace.findText = targetView.state.sliceDoc(
                        selection.from,
                        selection.to,
                    );
                }
                return true;
            },
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

    const isDark = document.documentElement.classList.contains("dark");
    const initialThemeExt = createTheme(
        isDark ? andromedaConfig : materialLightConfig,
    );

    session.state = EditorState.create({
        doc,
        extensions: [
            createSearchKeymap(),
            basicSetup,
            search({}),
            scrollPastEnd(),
            session.themeCompartment.of(initialThemeExt),
            session.fontSizeCompartment.of(createFontSizeExtension(editorState.fontSize)),
            session.langCompartment.of(getLanguageExtension(language)),
            session.wordWrapCompartment.of(
                editorState.wordWrap ? EditorView.lineWrapping : [],
            ),
            colorHints,
            contextMenuExtension,
            EditorView.contentAttributes.of({ spellcheck: "false" }),
            EditorView.updateListener.of((update) => {
                session.state = update.state;
                if (update.selectionSet || update.docChanged) {
                    syncBindings(session, update.state, update.view);
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

export function captureManagedEditorView(
    session: ManagedEditorSession,
    view: EditorView,
) {
    session.state = view.state;
    session.view = undefined;
}

export function disposeManagedEditorSession(session: ManagedEditorSession) {
    session.view = undefined;
    session.bindings = undefined;
    session.themeCompartment = undefined;
    session.fontSizeCompartment = undefined;
    session.langCompartment = undefined;
    session.wordWrapCompartment = undefined;
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
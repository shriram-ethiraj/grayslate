import { Compartment, EditorState } from "@codemirror/state";
import { redo } from "@codemirror/commands";
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
import { getMinimalTextChange } from "$lib/editor/core/csvCodeMirror";

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
    langCompartment?: Compartment;
    wordWrapCompartment?: Compartment;
    bindings?: SessionBindings;
};

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

export function dispatchManagedEditorTextChange(
    session: ManagedEditorSession,
    nextText: string,
    options?: {
        userEvent?: string;
        focus?: boolean;
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

    if (session.view) {
        session.view.dispatch({
            changes,
            userEvent: options?.userEvent ?? "input",
        });

        if (options?.focus !== false) {
            session.view.focus();
        }
        return true;
    }

    session.state = session.state.update({
        changes,
        userEvent: options?.userEvent ?? "input",
    }).state;
    syncBindings(session, session.state);
    return true;
}
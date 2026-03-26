import type { EditorView } from "codemirror";
import type { RecentFileSource } from "$lib/files/recentFiles";

export const DEFAULT_EDITOR_FONT_SIZE = 15;
export const MIN_EDITOR_FONT_SIZE = 10;
export const MAX_EDITOR_FONT_SIZE = 24;

function clampEditorFontSize(fontSize: number): number {
    return Math.min(MAX_EDITOR_FONT_SIZE, Math.max(MIN_EDITOR_FONT_SIZE, fontSize));
}

export type FileType =
    | "text"
    | "csv"
    | "markdown"
    | "json"
    | "javascript"
    | "typescript"
    | "python"
    | "html"
    | "css"
    | "yaml"
    | "c"
    | "cpp"
    | "java"
    | "go"
    | "xml"
    | "shell"
    | "cmd"
    | "dockerfile"
    | "auto";

export type EditorSurface = "editor" | "markdown-preview";
export type EditorPopupId =
    | "find-replace"
    | "go-to-line"
    | "language-picker"
    | "transformations";

export type EditorPopupOpenRequest =
    | {
        id: "find-replace";
        replaceMode: boolean;
    }
    | {
        id: "go-to-line";
    }
    | {
        id: "language-picker";
    }
    | {
        id: "transformations";
    };

type EditorPopupController = {
    open: (request: EditorPopupOpenRequest) => void;
    close: () => void;
};

const editorPopupControllers = new Map<EditorPopupId, EditorPopupController>();

export const editorState = $state<{
    activeView?: EditorView;
    activeSurface?: EditorSurface;
    popup: {
        active: EditorPopupId | undefined;
    };
    isUntitledDocument: boolean;
    isDirty: boolean;
    /** Absolute path of the file currently open in the editor, or undefined for untitled documents. */
    currentFilePath: string | undefined;
    /** Source classification of the current file: `"slates"` (managed notes directory) or `"local"` (external). */
    currentFileSource: RecentFileSource | undefined;
    currentDocumentLength: number;
    currentSelectionSize: number;
    fileType: FileType;
    fontSize: number;
    wordWrap: boolean;
    csv: {
        showTable: boolean;
        undo?: () => void;
        redo?: () => void;
        requestShowTable?: (showTable: boolean) => void | Promise<void>;
    };
    markdown: {
        showPreview: boolean;
    };
    loader: {
        visible: boolean;
        message: string;
        subMessage: string;
        /** 0-100. Use -1 for indeterminate (pulsing bar). */
        progress: number;
    };
    findReplace: {
        visible: boolean;
        replaceMode: boolean;
        findText: string;
        replaceText: string;
        matchCount: number;
        currentMatch: number;
        searching: boolean;
        caseSensitive: boolean;
        wholeWord: boolean;
        useRegex: boolean;
        searchError: string;
    };
    goToLine: {
        requestOpen?: () => boolean;
    };
}>({
    activeSurface: undefined,
    popup: {
        active: undefined,
    },
    isUntitledDocument: true,
    isDirty: false,
    currentFilePath: undefined,
    currentFileSource: undefined,
    currentDocumentLength: 0,
    currentSelectionSize: 0,
    fileType: "text",
    fontSize: DEFAULT_EDITOR_FONT_SIZE,
    wordWrap: false,
    csv: {
        showTable: false,
        undo: undefined,
        redo: undefined,
        requestShowTable: undefined,
    },
    markdown: {
        showPreview: true,
    },
    loader: {
        visible: false,
        message: "",
        subMessage: "",
        progress: -1,
    },
    findReplace: {
        visible: false,
        replaceMode: false,
        findText: "",
        replaceText: "",
        matchCount: 0,
        currentMatch: 0,
        searching: false,
        caseSensitive: false,
        wholeWord: false,
        useRegex: false,
        searchError: "",
    },
    goToLine: {
        requestOpen: undefined,
    },
});

function syncSelectionToFindReplace(): void {
    const view = editorState.activeView;
    if (!view) {
        editorState.findReplace.findText = "";
        return;
    }

    const selection = view.state.selection.main;
    if (selection.empty) {
        editorState.findReplace.findText = "";
        return;
    }

    editorState.findReplace.findText = view.state.sliceDoc(selection.from, selection.to);
}

export function registerEditorPopup(
    id: EditorPopupId,
    controller: EditorPopupController,
): () => void {
    editorPopupControllers.set(id, controller);

    return () => {
        if (editorPopupControllers.get(id) !== controller) {
            return;
        }

        editorPopupControllers.delete(id);
        if (editorState.popup.active === id) {
            editorState.popup.active = undefined;
        }
    };
}

export function syncEditorPopupOpenState(id: EditorPopupId, isOpen: boolean): void {
    if (isOpen) {
        editorState.popup.active = id;
        return;
    }

    if (editorState.popup.active === id) {
        editorState.popup.active = undefined;
    }
}

export function closeEditorPopup(id?: EditorPopupId): void {
    const targetId = id ?? editorState.popup.active;
    if (!targetId) {
        return;
    }

    if (editorState.popup.active === targetId) {
        editorState.popup.active = undefined;
    }

    editorPopupControllers.get(targetId)?.close();
}

export function openEditorPopup(request: EditorPopupOpenRequest): boolean {
    const controller = editorPopupControllers.get(request.id);
    if (!controller) {
        return false;
    }

    const activePopup = editorState.popup.active;
    if (activePopup && activePopup !== request.id) {
        closeEditorPopup(activePopup);
    }

    editorState.popup.active = request.id;
    controller.open(request);
    return true;
}

export function openFindReplacePanel(
    replaceMode: boolean,
    options: { seedSelection?: boolean } = {},
): boolean {
    if (editorState.csv.showTable) {
        return false;
    }

    if (options.seedSelection ?? true) {
        syncSelectionToFindReplace();
    }

    return openEditorPopup({
        id: "find-replace",
        replaceMode,
    });
}

export function openGoToLinePanel(): boolean {
    if (editorState.csv.showTable) {
        return false;
    }

    return editorState.goToLine.requestOpen?.() ?? false;
}

export function openLanguagePicker(): boolean {
    return openEditorPopup({ id: "language-picker" });
}

export function openTransformationsPalette(): boolean {
    if (editorState.csv.showTable) {
        return false;
    }

    return openEditorPopup({ id: "transformations" });
}

export function setEditorFontSize(fontSize: number): void {
    editorState.fontSize = clampEditorFontSize(fontSize);
}

export function increaseEditorFontSize(): void {
    setEditorFontSize(editorState.fontSize + 1);
}

export function decreaseEditorFontSize(): void {
    setEditorFontSize(editorState.fontSize - 1);
}

export function resetEditorFontSize(): void {
    editorState.fontSize = DEFAULT_EDITOR_FONT_SIZE;
}

/** Show the editor-area loader overlay with optional sub-message and progress. */
export function showEditorLoader(message: string, subMessage = "", progress = -1) {
    editorState.loader.visible = true;
    editorState.loader.message = message;
    editorState.loader.subMessage = subMessage;
    editorState.loader.progress = progress;
}

/** Update loader progress and labels without toggling visibility. */
export function updateEditorLoader(message: string, subMessage = "", progress = -1) {
    editorState.loader.message = message;
    editorState.loader.subMessage = subMessage;
    editorState.loader.progress = progress;
}

/** Hide the editor-area loader overlay. */
export function hideEditorLoader() {
    if (_graceTimeoutId !== undefined) {
        clearTimeout(_graceTimeoutId);
        _graceTimeoutId = undefined;
    }
    editorState.loader.visible = false;
    editorState.loader.message = "";
    editorState.loader.subMessage = "";
    editorState.loader.progress = -1;
}

// ---------------------------------------------------------------------------
// Decelerating progress ticker
// ---------------------------------------------------------------------------
// A single, reusable ticker that asymptotically approaches a ceiling value.
// Only one ticker can run at a time (there is only one loader overlay).

let _tickerId: ReturnType<typeof setInterval> | undefined;
let _graceTimeoutId: ReturnType<typeof setTimeout> | undefined;
let _tickerProgress = 0;

export interface LoaderTickerOptions {
    /** Upper bound the ticker approaches but never reaches (default 90). */
    ceiling?: number;
    /** Fraction of the remaining gap added per tick (default 0.06). */
    factor?: number;
    /** Minimum step per tick to keep the bar moving (default 0.3). */
    minStep?: number;
    /** Milliseconds between ticks (default 80). */
    interval?: number;
    /** Initial progress value (default 0). */
    startAt?: number;
    /**
     * Grace period in ms before the overlay becomes visible (default 150).
     * If the operation completes within this window the loader is never
     * shown, keeping fast transitions instant.
     */
    graceMs?: number;
}

/**
 * Start a decelerating progress ticker. Each tick adds a shrinking fraction
 * of the remaining gap to the ceiling, producing a smooth ease-out curve.
 *
 * The ticker only drives `editorState.loader.progress` — callers may update
 * `message` / `subMessage` independently at any time via direct property
 * assignment on `editorState.loader`.
 */
export function startLoaderTicker(
    message: string,
    subMessage = "",
    options: LoaderTickerOptions = {},
): void {
    stopLoaderTicker();

    const {
        ceiling = 90,
        factor = 0.06,
        minStep = 0.3,
        interval = 80,
        startAt = 0,
        graceMs = 150,
    } = options;

    _tickerProgress = startAt;

    // Pre-set message / subMessage / progress but do NOT make the overlay
    // visible yet.  It will appear only after the grace period elapses,
    // keeping fast operations instant.
    editorState.loader.message = message;
    editorState.loader.subMessage = subMessage;
    editorState.loader.progress = _tickerProgress;

    _graceTimeoutId = setTimeout(() => {
        _graceTimeoutId = undefined;
        editorState.loader.visible = true;
    }, graceMs);

    _tickerId = setInterval(() => {
        const remaining = ceiling - _tickerProgress;
        _tickerProgress += Math.max(minStep, remaining * factor);
        if (_tickerProgress >= ceiling - 0.1) _tickerProgress = ceiling - 0.1;
        editorState.loader.progress = _tickerProgress;
    }, interval);
}

/** Stop the active decelerating ticker, if any. */
export function stopLoaderTicker(): void {
    if (_graceTimeoutId !== undefined) {
        clearTimeout(_graceTimeoutId);
        _graceTimeoutId = undefined;
    }
    if (_tickerId !== undefined) {
        clearInterval(_tickerId);
        _tickerId = undefined;
    }
}

/**
 * Convenience: stop any active ticker, snap progress to 100 %, then hide the
 * loader overlay after `delay` ms. An optional `onComplete` callback fires
 * after the overlay is hidden.
 */
export function completeEditorLoader(
    message = "Done",
    subMessage = "",
    delay = 100,
    onComplete?: () => void,
): void {
    stopLoaderTicker();

    // If the loader overlay was never shown (operation completed within the
    // grace period), skip the animated completion and clean up immediately.
    if (!editorState.loader.visible) {
        hideEditorLoader();
        onComplete?.();
        return;
    }

    updateEditorLoader(message, subMessage, 100);
    setTimeout(() => {
        hideEditorLoader();
        onComplete?.();
    }, delay);
}

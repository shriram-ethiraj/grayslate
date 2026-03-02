import type { EditorView } from "codemirror";

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
    | "dockerfile"
    | "auto";

export const editorState = $state<{
    activeView?: EditorView;
    fileType: FileType;
    wordWrap: boolean;
    csv: {
        showTable: boolean;
        serializing: boolean;
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
}>({
    fileType: "text",
    wordWrap: false,
    csv: {
        showTable: false,
        serializing: false,
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
});

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

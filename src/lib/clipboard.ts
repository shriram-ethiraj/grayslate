import type { EditorView } from "codemirror";
import type { Text } from "@codemirror/state";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "$lib/components/ui/sonner";
import { editorState } from "$lib/state/editor.svelte";

export const LARGE_COPY_THRESHOLD_BYTES = 50 * 1024 * 1024;

// A one-million-code-unit slice encodes to at most 4 MiB of UTF-8, keeping
// every raw IPC message below the backend's bounded 8 MiB chunk limit.
const COPY_CHUNK_CODE_UNITS = 1024 * 1024;
const SLOW_COPY_FEEDBACK_DELAY_MS = 500;
const COPY_ID_HEADER = "x-grayslate-copy-id";
const COPY_INDEX_HEADER = "x-grayslate-copy-index";
const COPY_FINAL_HEADER = "x-grayslate-copy-final";
const COPY_CANCEL_HEADER = "x-grayslate-copy-cancel";

type ClipboardCopyResponse = {
    completed: boolean;
    byteLength: number;
};

type CsvClipboardCopyResponse = {
    version: number;
    byteLength: number;
};

let copyOperationInFlight = false;

class CopyFeedback {
    private toastId: string | number | undefined;
    private preparingTimer: ReturnType<typeof setTimeout> | undefined;
    private ownsProgressState = false;

    constructor(estimatedSize: number) {
        if (estimatedSize >= LARGE_COPY_THRESHOLD_BYTES) {
            this.showPreparing();
        } else {
            // CSV serialization can grow beyond its source text estimate due
            // to UTF-8 encoding and quoting. Treat elapsed time as a fallback
            // so every genuinely slow copy still gets progress feedback.
            this.preparingTimer = setTimeout(() => {
                this.preparingTimer = undefined;
                this.showPreparing();
            }, SLOW_COPY_FEEDBACK_DELAY_MS);
        }
    }

    noteBytes(byteLength: number): void {
        if (byteLength >= LARGE_COPY_THRESHOLD_BYTES) {
            this.showPreparing();
        }
    }

    succeed(byteLength: number): void {
        this.cancelPreparingTimer();
        if (this.toastId !== undefined) {
            toast.success(`Copied to clipboard · ${formatByteSize(byteLength)}`, {
                id: this.toastId,
            });
        }
        this.finishProgress();
    }

    fail(): void {
        this.cancelPreparingTimer();
        if (this.toastId === undefined) {
            toast.error("Failed to copy text");
        } else {
            toast.error("Failed to copy text", { id: this.toastId });
        }
        this.finishProgress();
    }

    private showPreparing(): void {
        this.cancelPreparingTimer();
        if (this.toastId !== undefined) {
            return;
        }
        this.toastId = toast.loading("Preparing copy…");
        this.ownsProgressState = true;
        editorState.copyInProgress = true;
    }

    private cancelPreparingTimer(): void {
        if (this.preparingTimer === undefined) {
            return;
        }
        clearTimeout(this.preparingTimer);
        this.preparingTimer = undefined;
    }

    private finishProgress(): void {
        if (!this.ownsProgressState) {
            return;
        }
        this.ownsProgressState = false;
        editorState.copyInProgress = false;
    }
}

function formatByteSize(byteLength: number): string {
    const mebibytes = byteLength / (1024 * 1024);
    return `${mebibytes.toFixed(mebibytes >= 100 ? 0 : 1)} MB`;
}

function isHighSurrogate(codeUnit: number): boolean {
    return codeUnit >= 0xd800 && codeUnit <= 0xdbff;
}

function chunkEnd(
    doc: Text,
    lineBreak: string,
    start: number,
    to: number,
): number {
    let end = Math.min(start + COPY_CHUNK_CODE_UNITS, to);
    if (end < to) {
        const finalCodeUnit = doc.sliceString(end - 1, end, lineBreak).charCodeAt(0);
        if (isHighSurrogate(finalCodeUnit)) {
            end -= 1;
        }
    }
    return end;
}

function copyHeaders(
    requestId: string,
    chunkIndex: number,
    finalChunk: boolean,
): Record<string, string> {
    return {
        [COPY_ID_HEADER]: requestId,
        [COPY_INDEX_HEADER]: String(chunkIndex),
        [COPY_FINAL_HEADER]: finalChunk ? "1" : "0",
    };
}

function cancelClipboardCopy(requestId: string): void {
    void invoke<ClipboardCopyResponse>(
        "clipboard_write_chunk",
        new Uint8Array(),
        {
            headers: {
                [COPY_ID_HEADER]: requestId,
                [COPY_CANCEL_HEADER]: "1",
            },
        },
    ).catch(() => {});
}

/**
 * Copy a CodeMirror range without materializing the whole range as one JS
 * string. Each slice is UTF-8 encoded and sent as a raw IPC body; Rust owns
 * assembly and the native clipboard write.
 */
export async function copyEditorRangeToClipboard(
    view: EditorView,
    from: number,
    to: number,
): Promise<boolean> {
    if (from >= to || copyOperationInFlight) {
        return false;
    }

    copyOperationInFlight = true;
    const feedback = new CopyFeedback(to - from);
    const requestId = crypto.randomUUID();
    const encoder = new TextEncoder();
    // CodeMirror Text values are immutable, so this is a cheap snapshot of the
    // exact content being copied even if the live editor changes mid-transfer.
    const doc = view.state.doc;
    const lineBreak = view.state.lineBreak;
    let offset = from;
    let chunkIndex = 0;
    let byteLength = 0;

    try {
        while (offset < to) {
            const end = chunkEnd(doc, lineBreak, offset, to);
            const chunk = encoder.encode(doc.sliceString(offset, end, lineBreak));
            byteLength += chunk.byteLength;
            feedback.noteBytes(byteLength);

            const finalChunk = end === to;
            const response = await invoke<ClipboardCopyResponse>(
                "clipboard_write_chunk",
                chunk,
                { headers: copyHeaders(requestId, chunkIndex, finalChunk) },
            );

            if (response.completed !== finalChunk) {
                throw new Error("Clipboard copy completed in an unexpected state.");
            }
            if (finalChunk && response.byteLength !== byteLength) {
                throw new Error(
                    `Clipboard copy size mismatch: expected ${byteLength}, wrote ${response.byteLength}.`,
                );
            }

            offset = end;
            chunkIndex += 1;
        }

        feedback.succeed(byteLength);
        if (editorState.activeView === view && view.dom.isConnected) {
            view.focus();
        }
        return true;
    } catch (error) {
        cancelClipboardCopy(requestId);
        feedback.fail();
        console.error("Clipboard copy failed:", error);
        return false;
    } finally {
        copyOperationInFlight = false;
    }
}

/** Serialize the authoritative Rust CSV session and copy it natively. */
export async function copyCsvSessionToClipboard(
    estimatedSize: number,
): Promise<boolean> {
    if (copyOperationInFlight) {
        return false;
    }

    copyOperationInFlight = true;
    const feedback = new CopyFeedback(estimatedSize);

    try {
        const response = await invoke<CsvClipboardCopyResponse>("csv_copy_to_clipboard");
        feedback.succeed(response.byteLength);
        return true;
    } catch (error) {
        feedback.fail();
        console.error("CSV clipboard copy failed:", error);
        return false;
    } finally {
        copyOperationInFlight = false;
    }
}

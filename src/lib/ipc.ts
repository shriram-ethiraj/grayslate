/**
 * Thin IPC helpers that sit between the frontend and Tauri's `invoke`.
 *
 * There are two transport patterns in the app:
 * - raw-byte responses for medium/large one-shot text payloads (for example
 *   file reads up to the backend-enforced 200 MB limit), and
 * - chunked channel delivery for very large text that would exceed WebView2's
 *   practical per-message IPC limits.
 *
 * This module centralizes both patterns so future Rust-backed workers can move
 * large text to the UI without each feature re-implementing chunk assembly.
 */

import { invoke } from "@tauri-apps/api/core";

const textDecoder = new TextDecoder("utf-8");

export type ChunkedTextEvent = {
    type: "chunk";
    /** Zero-based position of this chunk in the final text. */
    index: number;
    /** UTF-8 text slice for this chunk. */
    text: string;
};

type ChunkWaiter = {
    expectedCount: number;
    resolve: (chunks: string[]) => void;
    reject: (error: Error) => void;
};

/**
 * Reassembles large text delivered as a sequence of `chunk` channel events.
 *
 * The caller feeds chunks into `handleChunk()` as they arrive, then calls
 * `waitForChunks(expectedCount)` once the command's small JSON response tells
 * us how many chunks were emitted. The helper validates duplicate/out-of-range
 * chunks and only resolves once every chunk is present, returning the ordered
 * chunk list so the caller can materialize it as a string, a CodeMirror `Text`
 * rope, or any other large-text representation.
 */
export function createChunkedTextAccumulator() {
    let chunks: Array<string | undefined> = [];
    let receivedCount = 0;
    let failure: Error | undefined;
    let waiter: ChunkWaiter | undefined;

    function resetBuffers(): void {
        chunks = [];
        receivedCount = 0;
        failure = undefined;
    }

    function rejectWaiter(error: Error): void {
        if (!waiter) {
            return;
        }
        const activeWaiter = waiter;
        waiter = undefined;
        activeWaiter.reject(error);
    }

    function fail(message: string): void {
        const error = new Error(message);
        resetBuffers();
        failure = error;
        rejectWaiter(error);
    }

    function maybeResolveWaiter(): void {
        if (!waiter) {
            return;
        }

        if (failure) {
            rejectWaiter(failure);
            return;
        }

        const { expectedCount, resolve } = waiter;
        if (expectedCount === 0) {
            if (receivedCount > 0) {
                fail("Received text chunks for an empty transformation result.");
                return;
            }
            waiter = undefined;
            resetBuffers();
            resolve([]);
            return;
        }

        for (let index = expectedCount; index < chunks.length; index += 1) {
            if (chunks[index] !== undefined) {
                fail(
                    `Received out-of-range text chunk ${index + 1}; expected ${expectedCount} chunks.`,
                );
                return;
            }
        }

        if (receivedCount !== expectedCount) {
            return;
        }

        for (let index = 0; index < expectedCount; index += 1) {
            if (chunks[index] === undefined) {
                fail(
                    `Missing text chunk ${index + 1} of ${expectedCount}.`,
                );
                return;
            }
        }

        const orderedChunks = chunks.slice(0, expectedCount) as string[];
        waiter = undefined;
        resetBuffers();
        resolve(orderedChunks);
    }

    return {
        handleChunk(event: ChunkedTextEvent): void {
            // Once failed, ignore further chunks to avoid accumulating
            // unreachable data in memory for the rest of the stream.
            if (failure) {
                return;
            }

            if (!Number.isInteger(event.index) || event.index < 0) {
                fail("Received an invalid text chunk index.");
                return;
            }

            if (chunks[event.index] !== undefined) {
                fail(`Received duplicate text chunk ${event.index + 1}.`);
                return;
            }

            chunks[event.index] = event.text;
            receivedCount += 1;
            maybeResolveWaiter();
        },

        waitForChunks(expectedCount: number): Promise<string[]> {
            if (!Number.isInteger(expectedCount) || expectedCount < 0) {
                return Promise.reject(
                    new Error("Received an invalid chunk count for the transformation result."),
                );
            }

            if (failure) {
                return Promise.reject(failure);
            }

            if (waiter) {
                return Promise.reject(
                    new Error("Text chunk assembly already has a pending waiter."),
                );
            }

            return new Promise<string[]>((resolve, reject) => {
                waiter = { expectedCount, resolve, reject };
                maybeResolveWaiter();
            });
        },

        reset(): void {
            waiter = undefined;
            resetBuffers();
        },
    };
}

/**
 * Invoke a Tauri command whose Rust side returns `tauri::ipc::Response`
 * (raw bytes) and decode the response as a UTF-8 string.
 *
 * Use this instead of `invoke<string>()` for commands that transfer large
 * text payloads (e.g. `read_file_content`) to bypass JSON serialization.
 */
export async function invokeText(
    cmd: string,
    args?: Record<string, unknown>,
): Promise<string> {
    const buffer = await invoke<ArrayBuffer>(cmd, args);
    return textDecoder.decode(buffer);
}

export { invoke };

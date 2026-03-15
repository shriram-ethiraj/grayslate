/**
 * Thin IPC helpers that sit between the frontend and Tauri's `invoke`.
 *
 * Commands that move large text blobs (file reads) return raw bytes via
 * `tauri::ipc::Response` on the Rust side.  The standard `invoke<string>()`
 * path would JSON-serialise (and later JSON.parse) those blobs — this module
 * provides `invokeText()` which decodes the raw `ArrayBuffer` directly via
 * `TextDecoder`, avoiding that overhead entirely.
 *
 * Small-payload / structured commands should keep using the standard `invoke`
 * re-exported from this module.
 */

import { invoke } from "@tauri-apps/api/core";

const textDecoder = new TextDecoder("utf-8");

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

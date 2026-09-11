import { getCurrentWindow } from "@tauri-apps/api/window";
import { emitTo } from "@tauri-apps/api/event";
import { invoke } from "$lib/ipc";
import type { DocumentDescriptor } from "$lib/files/recentFiles";

export type CreateWindowResult =
  | { kind: "created"; windowLabel: string }
  | { kind: "focused-existing" };

export type OpenDisposition =
  | { kind: "open-here"; reservationId: string }
  | { kind: "focused-existing" }
  | { kind: "existing-owner" };

export type WindowLaunchIntent =
  | { kind: "primary-startup" }
  | { kind: "blank" }
  | {
      kind: "document";
      document: DocumentDescriptor;
      reservationId: string;
      lineNumber?: number;
    };

export async function emitToCurrentWindow<T>(event: string, payload?: T): Promise<void> {
  await emitTo(getCurrentWindow().label, event, payload);
}

export async function createBlankWindow(): Promise<CreateWindowResult> {
  return invoke<CreateWindowResult>("create_editor_window", {
    documentId: null,
    documentGeneration: null,
    lineNumber: null,
    focusExisting: true,
  });
}

export async function openDocumentInNewWindow(
  document: Pick<DocumentDescriptor, "documentId" | "generation">,
  lineNumber?: number,
  focusExisting = true,
): Promise<CreateWindowResult> {
  return invoke<CreateWindowResult>("create_editor_window", {
    documentId: document.documentId,
    documentGeneration: document.generation,
    lineNumber: lineNumber ?? null,
    focusExisting,
  });
}

export async function focusCurrentWindow(): Promise<void> {
  await invoke("focus_current_window");
}

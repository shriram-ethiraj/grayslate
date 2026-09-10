import { getCurrentWindow } from "@tauri-apps/api/window";
import { emitTo } from "@tauri-apps/api/event";
import { invoke } from "$lib/ipc";
import type { DocumentDescriptor } from "$lib/files/recentFiles";

export type CreateWindowResult =
  | { kind: "created"; windowLabel: string }
  | { kind: "focused-existing" };

export type OpenDisposition =
  | { kind: "open-here"; reservationId: string }
  | { kind: "focused-existing" };

export type WindowLaunchIntent =
  | { kind: "primary-startup" }
  | { kind: "blank" }
  | {
      kind: "document";
      document: DocumentDescriptor;
      reservationId: string;
    };

export interface PickDocumentIntoNewWindowResult {
  document: DocumentDescriptor;
  result: CreateWindowResult;
}

export async function emitToCurrentWindow<T>(event: string, payload?: T): Promise<void> {
  await emitTo(getCurrentWindow().label, event, payload);
}

export async function createBlankWindow(): Promise<CreateWindowResult> {
  return invoke<CreateWindowResult>("create_editor_window", {
    documentId: null,
    documentGeneration: null,
  });
}

export async function openDocumentInNewWindow(
  document: Pick<DocumentDescriptor, "documentId" | "generation">,
): Promise<CreateWindowResult> {
  return invoke<CreateWindowResult>("create_editor_window", {
    documentId: document.documentId,
    documentGeneration: document.generation,
  });
}

export async function pickDocumentIntoNewWindow(): Promise<PickDocumentIntoNewWindowResult | null> {
  const selected = await invoke<DocumentDescriptor | null>("pick_document");
  if (!selected) return null;
  return {
    document: selected,
    result: await openDocumentInNewWindow(selected),
  };
}

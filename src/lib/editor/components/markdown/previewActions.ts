import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "svelte-sonner";
import { editorState } from "$lib/state/editor.svelte";

let previewElement: HTMLElement | undefined;

function isNodeInsidePreview(node: Node | null, root: HTMLElement): boolean {
  if (!node) return false;
  return root.contains(node);
}

function getSelectionRangeInsidePreview(
  selection: Selection,
  root: HTMLElement,
): Range | null {
  for (let index = 0; index < selection.rangeCount; index += 1) {
    const range = selection.getRangeAt(index);
    if (
      isNodeInsidePreview(range.commonAncestorContainer, root) ||
      isNodeInsidePreview(range.startContainer, root) ||
      isNodeInsidePreview(range.endContainer, root)
    ) {
      return range;
    }
  }

  return null;
}

export function registerMarkdownPreviewElement(element: HTMLElement): void {
  previewElement = element;
}

export function unregisterMarkdownPreviewElement(element: HTMLElement): void {
  if (previewElement === element) {
    previewElement = undefined;
  }
}

export function focusMarkdownPreview(): void {
  if (!previewElement) return;
  previewElement.focus({ preventScroll: true });
}

export function activateMarkdownPreview(): void {
  editorState.activeSurface = "markdown-preview";
}

export function isMarkdownPreviewActive(): boolean {
  return editorState.activeSurface === "markdown-preview";
}

export function getMarkdownPreviewSelectionText(): string {
  if (!previewElement) return "";

  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
    return "";
  }

  const previewRange = getSelectionRangeInsidePreview(selection, previewElement);
  if (!previewRange) return "";

  return previewRange.toString();
}

export function getMarkdownPreviewAllText(): string {
  if (!previewElement) return "";

  const range = document.createRange();
  range.selectNodeContents(previewElement);
  return range.toString();
}

export function hasMarkdownPreviewSelection(): boolean {
  return getMarkdownPreviewSelectionText().length > 0;
}

export async function copyMarkdownPreviewSelection(): Promise<boolean> {
  const text = getMarkdownPreviewSelectionText();
  if (!text) return false;

  try {
    await writeText(text);
    focusMarkdownPreview();
    return true;
  } catch {
    toast.error("Failed to copy text");
    return false;
  }
}

export async function copyMarkdownPreviewAll(): Promise<boolean> {
  const text = getMarkdownPreviewAllText();
  if (!text) return false;

  try {
    await writeText(text);
    focusMarkdownPreview();
    return true;
  } catch {
    toast.error("Failed to copy text");
    return false;
  }
}

export async function copyMarkdownPreviewSelectionOrAll(): Promise<boolean> {
  if (hasMarkdownPreviewSelection()) {
    return copyMarkdownPreviewSelection();
  }

  return copyMarkdownPreviewAll();
}

export function selectAllMarkdownPreview(): boolean {
  if (!previewElement) return false;

  const selection = window.getSelection();
  if (!selection) return false;

  const range = document.createRange();
  range.selectNodeContents(previewElement);
  selection.removeAllRanges();
  selection.addRange(range);
  activateMarkdownPreview();
  focusMarkdownPreview();
  return true;
}
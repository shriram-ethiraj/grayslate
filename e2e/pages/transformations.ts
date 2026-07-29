import { $$ } from "@wdio/globals";
import { TIMEOUTS, INTERVALS } from "../config/timeouts.js";
import { pressMod } from "../driver/keys.js";
import { waitFor } from "../driver/wait.js";
import {
  byTestId,
  clickTestId,
  isAriaDisabled,
  isDisplayedTestId,
  waitForTestId,
} from "./common.js";
import { focus } from "./editor.js";

/** The transformations palette and the toasts its actions raise. */

export async function openPalette(focusEditorFirst = true): Promise<void> {
  if (focusEditorFirst) await focus();
  await pressMod("k");
  await waitForTestId("transformations-palette");
}

export async function closePalette(): Promise<void> {
  await waitForTestId("transformations-palette", { reverse: true });
}

export async function search(query: string): Promise<void> {
  const input = await byTestId("transformations-search");
  await input.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The transformations search input never appeared.",
  });
  await input.setValue(query);
}

/** The action ids currently listed in the palette. */
export async function visibleActionIds(): Promise<string[]> {
  const items = await $$("[data-testid^='transform-item-']");
  const ids: string[] = [];
  for (const item of items) {
    if (!(await item.isDisplayed().catch(() => false))) continue;
    const testId = await item.getAttribute("data-testid");
    if (testId) ids.push(testId.replace("transform-item-", ""));
  }
  return ids;
}

/** Open the palette and run one action by id. */
export async function run(actionId: string, focusEditorFirst = true): Promise<void> {
  await openPalette(focusEditorFirst);
  const item = `transform-item-${actionId}`;
  await waitForTestId(item, { timeoutMs: TIMEOUTS.ui });
  await clickTestId(item);
}

/** Whether the palette control is unavailable (it is blocked in CSV table mode). */
export async function isPaletteActionDisabled(): Promise<boolean> {
  return isAriaDisabled("action-transformations");
}

// ── Toasts ─────────────────────────────────────────────────────────────────

/** Visible toast messages, text only (the icon is excluded). */
export async function visibleToasts(): Promise<string[]> {
  const messages = await $$("[data-testid='toast-message']");
  const texts: string[] = [];
  for (const message of messages) {
    if (!(await message.isDisplayed().catch(() => false))) continue;
    texts.push((await message.getText()).trim());
  }
  return texts;
}

/** Wait for a toast whose text exactly equals `expected`. */
export async function waitForToast(expected: string): Promise<void> {
  let observed: string[] = [];
  await waitFor(
    async () => {
      observed = await visibleToasts();
      return observed.includes(expected);
    },
    {
      message: () => `No visible toast matched ${JSON.stringify(expected)}. Last observed: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.ui,
      intervalMs: INTERVALS.fast,
    },
  );
}

/** Wait for a toast containing `fragment`, for messages that embed a path. */
export async function waitForToastContaining(fragment: string): Promise<void> {
  let observed: string[] = [];
  await waitFor(
    async () => {
      observed = await visibleToasts();
      return observed.some((text) => text.includes(fragment));
    },
    {
      message: () => `No visible toast contained ${JSON.stringify(fragment)}. Last observed: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.ui,
      intervalMs: INTERVALS.fast,
    },
  );
}

/** The loader shown during long transformations, file reads, and CSV flushes. */
export const loader = {
  async isVisible(): Promise<boolean> {
    return isDisplayedTestId("editor-loader");
  },
  async waitForVisible(): Promise<void> {
    await waitForTestId("editor-loader", { timeoutMs: TIMEOUTS.heavy });
  },
  async waitForHidden(): Promise<void> {
    await waitForTestId("editor-loader", { reverse: true, timeoutMs: TIMEOUTS.heavy });
  },
  /** Cancel the in-flight operation the loader represents. */
  async cancel(): Promise<void> {
    await clickTestId("editor-loader-cancel");
  },
};

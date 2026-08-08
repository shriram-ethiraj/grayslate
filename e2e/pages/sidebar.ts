import { $ } from "@wdio/globals";
import { TIMEOUTS, INTERVALS } from "../config/timeouts.js";
import { clickSelector, dismissTransientOverlays } from "../driver/interact.js";
import { readSidebarPaneSize, readSidebarPaths } from "../driver/probe.js";
import { waitFor, WaitTimeoutError } from "../driver/wait.js";
import {
  attributeOf,
  byTestId,
  clickTestId,
  pressEscape,
  waitForTestId,
} from "./common.js";

/** The library sidebar: recent files, filters, sorting, search, file actions. */

export type FilterTab = "unified" | "slates" | "local";
export type SortMode =
  | "recently-opened"
  | "least-recently-opened"
  | "name-asc"
  | "name-desc"
  | "size-desc"
  | "size-asc";

/**
 * Whether the sidebar is expanded.
 *
 * The sidebar's children remain mounted inside a zero-width, overflow-hidden
 * pane when it closes. WebDriver therefore reports those clipped controls as
 * displayed even though a user cannot see them. Paneforge exposes the pane's
 * actual size on the separator's standard `aria-valuenow`, which gives us an
 * atomic state read without retaining a stale element reference.
 */
export async function isOpen(): Promise<boolean> {
  const paneSize = await readSidebarPaneSize();
  return paneSize !== null && paneSize > 0;
}

async function ensureOpenState(expectedOpen: boolean): Promise<void> {
  let lastError: unknown;

  for (let attempt = 1; attempt <= 3; attempt += 1) {
    // Re-read before every attempt. If a delayed first click finally applied,
    // this prevents a second click from toggling the sidebar back.
    if ((await isOpen()) === expectedOpen) return;

    try {
      await clickTestId("sidebar-toggle");
      await waitFor(async () => (await isOpen()) === expectedOpen, {
        message: expectedOpen
          ? "The sidebar never expanded after toggling it."
          : "The sidebar never collapsed after toggling it.",
        timeoutMs: TIMEOUTS.ui,
        intervalMs: INTERVALS.fast,
      });
      return;
    } catch (error) {
      lastError = error;
      // Retry only when the click completed but its postcondition did not.
      // Driver/session failures must remain immediate and visible.
      if (!(error instanceof WaitTimeoutError) || attempt === 3) throw error;
      await dismissTransientOverlays();
    }
  }

  throw lastError;
}

export async function ensureOpen(): Promise<void> {
  await ensureOpenState(true);
}

export async function ensureClosed(): Promise<void> {
  await ensureOpenState(false);
}

/** Horizontal position of the persisted sidebar divider, in viewport pixels. */
export async function dividerX(): Promise<number> {
  const handle = await byTestId("sidebar-resize-handle");
  await handle.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The sidebar resize handle never became visible.",
  });
  return handle.getLocation("x");
}

/** Resize the open sidebar by a real pointer drag. */
export async function resizeBy(deltaX: number): Promise<void> {
  const handle = await byTestId("sidebar-resize-handle");
  await handle.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The sidebar resize handle never became visible.",
  });
  await handle.dragAndDrop({ x: deltaX, y: 0 }, { duration: 400 });
}

/** The card element for a file path. */
export function card(filePath: string): ReturnType<typeof $> {
  return $(`[data-card-path="${filePath}"]`);
}

export async function waitForCard(filePath: string, present = true): Promise<void> {
  const element = await card(filePath);
  await element.waitForDisplayed({
    reverse: !present,
    timeout: TIMEOUTS.disk,
    timeoutMsg: present
      ? `Sidebar card for ${filePath} never appeared.`
      : `Sidebar card for ${filePath} was still present.`,
  });
}

/** Open a file by clicking its card. */
export async function openCard(filePath: string): Promise<void> {
  const element = await card(filePath);
  await element.waitForDisplayed({
    timeout: TIMEOUTS.disk,
    timeoutMsg: `Sidebar card for ${filePath} never appeared.`,
  });
  await clickSelector(`[data-card-path="${filePath}"] button`);
}

/** Choose one action from a card's overflow menu. */
export async function cardAction(
  filePath: string,
  action:
    | "open"
    | "reveal"
    | "copy-path"
    | "duplicate"
    | "duplicate-as-slate"
    | "unlink"
    | "rename"
    | "delete",
): Promise<void> {
  const element = await card(filePath);
  await element.waitForDisplayed({
    timeout: TIMEOUTS.disk,
    timeoutMsg: `Sidebar card for ${filePath} never appeared.`,
  });

  // Clear anything the previous action left open *before* the first attempt,
  // not just on retry. A hover tooltip over the overflow button intercepts the
  // click, and the symptom surfaces much later as "the rename input never
  // appeared" — several steps away from the cause.
  await pressEscape();
  await dismissTransientOverlays();

  // Route through the driver's reacquisition path: a raw scoped click skips the
  // retry that handles a tooltip left open over the overflow button.
  await clickSelector(`[data-card-path="${filePath}"] [data-testid='sidebar-file-options']`);
  const item = `sidebar-action-${action}`;
  await waitForTestId(item);
  await clickTestId(item);
}

export async function setFilterTab(tab: FilterTab): Promise<void> {
  const testId = `sidebar-tab-${tab}`;
  let lastError: unknown;

  for (let attempt = 1; attempt <= 3; attempt += 1) {
    await clickTestId(testId);
    try {
      // WebKit can report a successful click while a tooltip or a shifting tab
      // makes the pointer land on an adjacent trigger. The list container
      // already exists in that case, so waiting for it alone proves nothing.
      // Require the requested tab's own active state before callers inspect the
      // filtered cards.
      await waitFor(async () => (await attributeOf(testId, "data-state")) === "active", {
        message: `The ${tab} sidebar filter did not become active.`,
        timeoutMs: TIMEOUTS.ui,
        intervalMs: INTERVALS.fast,
      });
      await waitForTestId("sidebar-file-list");
      return;
    } catch (error) {
      lastError = error;
      if (!(error instanceof WaitTimeoutError) || attempt === 3) throw error;
      await dismissTransientOverlays();
    }
  }

  throw lastError;
}

export async function setSort(mode: SortMode): Promise<void> {
  await clickTestId("sidebar-sort-trigger");
  await clickTestId(`sidebar-sort-${mode}`);
}

/** Visible card paths in their current rendered order. */
export async function visiblePaths(): Promise<string[]> {
  return readSidebarPaths();
}

export async function waitForPaths(
  predicate: (paths: string[]) => boolean,
  message: string,
): Promise<void> {
  let observed: string[] = [];
  await waitFor(
    async () => {
      observed = await visiblePaths();
      return predicate(observed);
    },
    {
      message: () => `${message} Last observed: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.disk,
      intervalMs: INTERVALS.slow,
    },
  );
}

// ── Search ─────────────────────────────────────────────────────────────────

export async function search(query: string): Promise<void> {
  const input = await byTestId("sidebar-search-input");
  await input.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The sidebar search input never appeared.",
  });
  await input.click();
  await input.setValue(query);
}

export async function clearSearch(): Promise<void> {
  await clickTestId("sidebar-clear-search");
}

export async function searchValue(): Promise<string> {
  return (await byTestId("sidebar-search-input")).getValue();
}

export async function toggleSearchOption(
  option: "case" | "word" | "regex",
): Promise<void> {
  await clickTestId(`sidebar-search-${option}`);
}

export async function searchOptionPressed(
  option: "case" | "word" | "regex",
): Promise<boolean> {
  return (await attributeOf(`sidebar-search-${option}`, "aria-pressed")) === "true";
}

export async function refresh(): Promise<void> {
  await clickTestId("sidebar-refresh");
}

import { $, browser } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";

/**
 * Interaction with bounded reacquisition.
 *
 * A passing run of the suite still logged ~70 `element not interactable`,
 * `element click intercepted`, and stale-reference failures, plus 8 "CodeMirror
 * content element is missing". Those are not harmless: they mean an element
 * reference was captured before Svelte finished replacing the node, and the
 * test only survived because some enclosing poll happened to retry. Left alone,
 * they are exactly the conditions that turn into intermittent CI failures.
 *
 * The fix is to re-find the element and retry, but only for the specific
 * transient WebDriver conditions, only a bounded number of times, and always
 * visibly. Anything else propagates immediately — a retry loop that swallows
 * real failures is worse than the race it hides.
 */

const TRANSIENT_PATTERNS = [
  "stale element reference",
  "element click intercepted",
  "element not interactable",
  "node with given id does not belong to the document",
  "no such element",
];

const MAX_ATTEMPTS = 3;

export function isTransientWebDriverError(error: unknown): boolean {
  const message = String((error as Error)?.message ?? error).toLowerCase();
  return TRANSIENT_PATTERNS.some((pattern) => message.includes(pattern));
}

/** Retries are reported so a rising count is visible rather than silent. */
const retryLog: { selector: string; attempt: number; reason: string }[] = [];

export function readRetryLog(): readonly { selector: string; attempt: number; reason: string }[] {
  return retryLog;
}

export function clearRetryLog(): void {
  retryLog.length = 0;
}

/**
 * Move the pointer out of the way and wait for hover overlays to close.
 *
 * Tooltips in this app render above their trigger, so a tooltip opened by one
 * action routinely covers the control the next action needs to press. That
 * shows up as `element click intercepted` — which the suite's warning log was
 * already full of before anything retried it.
 */
export async function dismissTransientOverlays(): Promise<void> {
  await browser.action("pointer").move({ x: 0, y: 0 }).perform();
  await browser
    .waitUntil(
      async () =>
        browser.execute(() => {
          // The overlay that actually intercepts a click is the positioned
          // popover wrapper, not the inner `role="tooltip"` span — that span is
          // a description node and can measure zero even while its wrapper
          // covers the control. Checking only the span made this dismissal a
          // no-op for the exact case it was written for.
          const overlays = document.querySelectorAll(
            "[data-slot='tooltip-content'], [role='tooltip'], [data-slot='dropdown-menu-content']",
          );
          return Array.from(overlays).every(
            (overlay) => (overlay as HTMLElement).getClientRects().length === 0,
          );
        }),
      {
        timeout: 2_000,
        interval: 50,
        timeoutMsg: "Overlays stayed open after moving the pointer away.",
      },
    )
    .catch(() => {
      // Best-effort: if an overlay is genuinely stuck, the click retry will
      // fail with the original interception error, which is more useful than
      // failing here.
    });
}

/**
 * Run `action` against a freshly located element, retrying only on the
 * transient conditions above.
 */
export async function withFreshElement<T>(
  selector: string,
  action: (element: Awaited<ReturnType<typeof $>>) => Promise<T>,
  options?: { timeoutMs?: number },
): Promise<T> {
  const timeoutMs = options?.timeoutMs ?? TIMEOUTS.ui;
  let lastError: unknown;

  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    try {
      const element = await $(selector);
      // Wait for *displayed*, not `isClickable`. `isClickable` additionally
      // requires the element to be the topmost node at its centre point, which
      // is false for any control wrapped in a tooltip trigger even though a
      // real click lands on it perfectly well. Being too strict here is what
      // pushed earlier specs into scripted `.focus()` workarounds that bypassed
      // the click path entirely. Genuine interception still surfaces below as a
      // transient error and is retried.
      await element.waitForDisplayed({
        timeout: timeoutMs,
        timeoutMsg: `'${selector}' never became visible.`,
      });
      return await action(element);
    } catch (error) {
      lastError = error;
      if (!isTransientWebDriverError(error) || attempt === MAX_ATTEMPTS) throw error;

      const reason = String((error as Error)?.message ?? error).split("\n")[0];
      retryLog.push({ selector, attempt, reason });
      console.warn(
        `[e2e] reacquiring '${selector}' after transient failure ` +
          `(attempt ${attempt}/${MAX_ATTEMPTS}): ${reason}`,
      );

      // The usual interceptor is a tooltip left open by the previous action,
      // sitting over the control we are trying to press. Park the pointer
      // somewhere harmless and wait for the overlays to close, rather than
      // immediately re-clicking into the same obstruction.
      await dismissTransientOverlays();

      // Let any pending re-render settle before re-finding the node.
      await browser.waitUntil(async () => (await $(selector)).isExisting(), {
        timeout: timeoutMs,
        interval: 50,
        timeoutMsg: `'${selector}' disappeared while retrying.`,
      });
    }
  }

  throw lastError;
}

/** Click an element by `data-testid`, tolerating a mid-render re-mount. */
export async function clickTestId(testId: string): Promise<void> {
  await withFreshElement(`[data-testid='${testId}']`, async (element) => {
    await element.click();
  });
}

/** Click any element by raw selector, with the same reacquisition guarantees. */
export async function clickSelector(selector: string): Promise<void> {
  await withFreshElement(selector, async (element) => {
    await element.click();
  });
}

/** Right-click an element by `data-testid`. */
export async function rightClickTestId(testId: string): Promise<void> {
  await withFreshElement(`[data-testid='${testId}']`, async (element) => {
    await element.click({ button: "right" });
  });
}

/** Set an input's value through the real input path. */
export async function setValueTestId(testId: string, value: string): Promise<void> {
  await withFreshElement(`[data-testid='${testId}']`, async (element) => {
    await element.setValue(value);
  });
}

/** Clear an input, re-resolving it first so a re-render cannot stale the handle. */
export async function clearValueTestId(testId: string): Promise<void> {
  await withFreshElement(`[data-testid='${testId}']`, async (element) => {
    await element.clearValue();
  });
}

/**
 * Hover an element with a real pointer move.
 *
 * The suite used to dispatch a synthetic `pointerenter` after `moveTo()`,
 * which meant the tooltip's actual trigger path was never exercised — a
 * regression in the hover wiring would not have failed anything. A genuine
 * move also needs the pointer to start somewhere else, or WebKit may consider
 * it already inside the element and emit no enter event at all.
 */
export async function hoverTestId(testId: string): Promise<void> {
  const selector = `[data-testid='${testId}']`;
  await withFreshElement(selector, async (element) => {
    // Park the pointer away from the target first so the move crosses a real
    // boundary and produces an enter.
    await browser.action("pointer").move({ x: 0, y: 0 }).perform();
    await element.moveTo();
  });
}

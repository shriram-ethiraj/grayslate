import { $, browser } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { ESCAPE } from "../driver/keys.js";
import { waitFor } from "../driver/wait.js";
import {
  clickTestId as clickWithRetry,
  isTransientWebDriverError,
  rightClickTestId as rightClickWithRetry,
  setValueTestId as setValueWithRetry,
} from "../driver/interact.js";

/**
 * Shared element plumbing for the page objects.
 *
 * Page objects model *what a user can do* on one surface. They act and they
 * read; they never assert. Keeping assertions in the specs is what lets a
 * failure message name the behavior under test rather than the helper.
 */

/** An element addressed by its `data-testid`. */
export function byTestId(testId: string): ReturnType<typeof $> {
  return $(`[data-testid='${testId}']`);
}

/**
 * Click an element by `data-testid`.
 *
 * Goes through `driver/interact.ts` so a click that lands mid-re-render is
 * retried against a freshly located node instead of failing on a stale
 * reference. Retries are logged, never silent.
 */
export const clickTestId = clickWithRetry;

/** Right-click an element by `data-testid`, to open its context menu. */
export const rightClickTestId = rightClickWithRetry;

/** Type into an input by `data-testid`, through the real input path. */
export const setValueTestId = setValueWithRetry;

/** Whether an element carrying this `data-testid` is present in the DOM. */
export async function existsTestId(testId: string): Promise<boolean> {
  return (await byTestId(testId)).isExisting();
}

/** Whether an element carrying this `data-testid` is visible. */
export async function isDisplayedTestId(testId: string): Promise<boolean> {
  try {
    return await (await byTestId(testId)).isDisplayed();
  } catch (error) {
    if (isTransientWebDriverError(error)) return false;
    throw error;
  }
}

/** Read one attribute, or null when the element is absent. */
export async function attributeOf(
  testId: string,
  attribute: string,
): Promise<string | null> {
  const element = await byTestId(testId);
  if (!(await element.isExisting())) return null;
  return element.getAttribute(attribute);
}

/** Read the trimmed visible text of an element. */
export async function textOf(testId: string): Promise<string> {
  return (await byTestId(testId)).getText();
}

/** Wait for a testid'd element to appear, with a message that names it. */
export async function waitForTestId(
  testId: string,
  options?: { reverse?: boolean; timeoutMs?: number },
): Promise<void> {
  const reverse = options?.reverse ?? false;
  await waitFor(
    async () => {
      try {
        const element = await byTestId(testId);
        if (!(await element.isExisting())) return reverse;
        const displayed = await element.isDisplayed();
        return reverse ? !displayed : displayed;
      } catch (error) {
        // A remount is not proof that a closing control disappeared: it may
        // already have a visible replacement. Reacquire on the next poll for
        // both open and close waits.
        if (isTransientWebDriverError(error)) return false;
        throw error;
      }
    },
    {
      message: reverse
        ? `'${testId}' was still displayed.`
        : `'${testId}' never became displayed.`,
      timeoutMs: options?.timeoutMs ?? TIMEOUTS.ui,
    },
  );
}

/** Whether a control is disabled.
 *
 * Grayslate's `TooltipButton` disables via `aria-disabled` only and never sets
 * the native `disabled` attribute, so WebDriver's `isEnabled()` reports true
 * regardless of state. Always read the ARIA state instead.
 */
export async function isAriaDisabled(testId: string): Promise<boolean> {
  return (await attributeOf(testId, "aria-disabled")) === "true";
}

/** Dismiss the topmost dialog or menu. */
export async function pressEscape(): Promise<void> {
  await browser.keys(ESCAPE);
}

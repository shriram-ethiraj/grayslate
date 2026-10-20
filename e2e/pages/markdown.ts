import { $ } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { isSelectorUncovered, readComputedStyleBySelector, readWindowGlobal } from "../driver/probe.js";
import { waitFor, waitForAppStable } from "../driver/wait.js";
import { dismissTransientOverlays } from "../driver/interact.js";
import { attributeOf, clickTestId, isDisplayedTestId, waitForTestId } from "./common.js";
import { waitUntilReady } from "./editor.js";

/**
 * The Markdown preview pane.
 *
 * Rendering and sanitization happen in Rust (`pulldown-cmark` + `ammonia`), so
 * the preview's DOM is the sanitizer's output — assert on it directly, and on
 * the *absence of side effects* rather than only on stripped markup.
 */

export function pane(): ReturnType<typeof $> {
  return $("[data-testid='markdown-preview']");
}

export async function setVisible(visible: boolean): Promise<ReturnType<typeof $>> {
  await waitUntilReady({ language: "markdown" });
  const desired = String(visible);

  await requestVisible(visible);

  await waitFor(
    async () => {
      const pressed = await attributeOf("action-toggle-preview", "aria-pressed");
      const displayed = await isDisplayedTestId("markdown-preview");
      return pressed === desired && displayed === visible;
    },
    {
      message: `Markdown preview never became ${visible ? "visible" : "hidden"}.`,
      timeoutMs: TIMEOUTS.editor,
    },
  );

  return pane();
}

/** Click the preview toggle without waiting for a render to settle. */
export async function requestVisible(visible: boolean): Promise<void> {
  // Opening needs a settled editor. Closing deliberately must not: cancellation
  // tests call this while render_markdown_preview is blocked in Rust, and an
  // idle barrier here would wait for the very operation the click must cancel.
  if (visible) {
    await waitUntilReady({ language: "markdown" });
    await waitForAppStable();
  }
  if ((await attributeOf("action-toggle-preview", "aria-pressed")) === String(visible)) return;
  // File teardown or a preceding hover tooltip can still cover the toggle
  // after document readiness. Observe the real hit target before clicking.
  await dismissTransientOverlays();
  await waitFor(() => isSelectorUncovered("[data-testid='action-toggle-preview']"), {
    timeoutMs: TIMEOUTS.ui,
    message: "The Markdown preview toggle remained covered by another surface.",
  });
  await clickTestId("action-toggle-preview");
}

export async function waitForRendered(): Promise<void> {
  await waitForTestId("markdown-preview", { timeoutMs: TIMEOUTS.editor });
}

/** All rendered elements matching a selector inside the preview. */
export function within(selector: string): ReturnType<typeof $> {
  return $(`[data-testid='markdown-preview'] ${selector}`);
}

export async function scrollTop(): Promise<number> {
  const element = await pane();
  return Number(await element.getProperty("scrollTop"));
}

export async function scrollHeight(): Promise<number> {
  const element = await pane();
  return Number(await element.getProperty("scrollHeight"));
}

export async function clientHeight(): Promise<number> {
  const element = await pane();
  return Number(await element.getProperty("clientHeight"));
}

/** Wait until the preview scroll position satisfies a predicate. */
export async function waitForScroll(
  predicate: (top: number) => boolean,
  message: string,
): Promise<void> {
  let observed = 0;
  await waitFor(
    async () => {
      observed = await scrollTop();
      return predicate(observed);
    },
    {
      message: () => `${message} Last observed scrollTop: ${observed}`,
      timeoutMs: TIMEOUTS.ui,
    },
  );
}

export async function openContextMenu(): Promise<void> {
  const element = await pane();
  await element.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The preview was not visible when opening its context menu.",
  });
  await element.click({ button: "right" });
  await waitForTestId("markdown-context-menu");
}

export async function chooseContextMenuItem(item: "copy" | "select-all"): Promise<void> {
  await clickTestId(`markdown-context-${item}`);
}

export { readComputedStyleBySelector as computedStyle, readWindowGlobal as windowGlobal };

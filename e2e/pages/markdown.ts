import { $ } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { readComputedStyleBySelector, readWindowGlobal } from "../driver/probe.js";
import { waitFor } from "../driver/wait.js";
import { attributeOf, byTestId, clickTestId, isDisplayedTestId, waitForTestId } from "./common.js";
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

  const toggle = await byTestId("action-toggle-preview");
  await toggle.waitForClickable({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The preview toggle never became clickable.",
  });
  if ((await toggle.getAttribute("aria-pressed")) !== desired) {
    await toggle.click();
  }

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
  await waitUntilReady({ language: "markdown" });
  const desired = String(visible);
  const toggle = await byTestId("action-toggle-preview");
  await toggle.waitForClickable({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The preview toggle never became clickable.",
  });
  if ((await toggle.getAttribute("aria-pressed")) !== desired) {
    await toggle.click();
  }
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

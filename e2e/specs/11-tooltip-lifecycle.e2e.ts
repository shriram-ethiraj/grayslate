import { browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import {
  clickTestId,
  dismissTransientOverlays,
  hoverSelectorByGeometry,
  hoverTextByGeometry,
  hoverTestId,
} from "../driver/interact.js";
import { pressModShift, releaseModifiers, SHIFT, TAB } from "../driver/keys.js";
import {
  isSelectorVisible,
  readFoldGutterTooltipVisible,
  readFocusedTestId,
  readTooltipWindowActive,
  readVisibleTooltips,
} from "../driver/probe.js";
import { invokeInApp } from "../driver/invoke.js";
import {
  requireConditionForDuration,
  waitFor,
} from "../driver/wait.js";
import { openText } from "../fixtures/factories.js";
import { attributeOf } from "../pages/common.js";
import * as editor from "../pages/editor.js";

const SHARED_TOOLTIP_SELECTOR = "[data-slot='tooltip-content']";
const CODEMIRROR_HOVER_SELECTOR = ".cm-tooltip.cm-tooltip-hover";
const FOLD_GUTTER_SELECTOR =
  ".cm-foldGutter .cm-gutterElement:not(:first-child) span";

async function waitForWindowCount(expected: number): Promise<string[]> {
  let handles: string[] = [];
  await waitFor(
    async () => {
      handles = await browser.getWindowHandles();
      return handles.length === expected;
    },
    {
      message: () =>
        `Expected ${expected} Grayslate window(s). Last handles: ${JSON.stringify(handles)}`,
    },
  );
  return handles;
}

async function deactivateMainWindow(mainHandle: string): Promise<string> {
  await pressModShift("n");
  await releaseModifiers();

  const handles = await waitForWindowCount(2);
  const secondaryHandle = handles.find((handle) => handle !== mainHandle);
  if (!secondaryHandle) throw new Error("The new Grayslate window had no distinct handle.");

  await waitFor(
    async () => !(await readTooltipWindowActive()),
    { message: "The original webview never observed native window deactivation." },
  );
  return secondaryHandle;
}

async function closeSecondaryAndRestoreMain(
  mainHandle: string,
  secondaryHandle: string,
): Promise<void> {
  await browser.switchToWindow(secondaryHandle);
  await browser.closeWindow();
  await waitForWindowCount(1);
  await browser.switchToWindow(mainHandle);
  await invokeInApp<void>("e2e_focus_window");
  await editor.waitUntilReady();
  await waitFor(readTooltipWindowActive, {
    message: "The original webview never observed native window reactivation.",
  });
}

async function requireTooltipToStayClosed(
  predicate: () => Promise<boolean>,
  message: string,
): Promise<void> {
  await requireConditionForDuration(async () => !(await predicate()), {
    durationMs: 700,
    message,
  });
}

describe("Tooltip window lifecycle", () => {
  scenario(
    "shell.tooltips.window-lifecycle",
    "dismisses shared, CSS, and CodeMirror tooltips across native focus changes",
    async () => {
      const [mainHandle] = await waitForWindowCount(1);
      if (!mainHandle) throw new Error("The initial Grayslate window handle is missing.");

      await openText(
        "tooltip-window-lifecycle.json",
        '{\n  "tooltip": {\n    "value": 1\n  }\n}\n',
      );

      // Exercise CodeMirror before the first native window round-trip. Under
      // Xvfb, WebKit can stop painting content text after that transition even
      // though CodeMirror's state and gutter remain live.
      await hoverTextByGeometry(".cm-content", '"tooltip"');
      await waitFor(
        async () => isSelectorVisible(CODEMIRROR_HOVER_SELECTOR),
        { message: "The CodeMirror path tooltip never became visible." },
      );
      let secondaryHandle = await deactivateMainWindow(mainHandle);
      expect(await isSelectorVisible(CODEMIRROR_HOVER_SELECTOR)).toBe(false);
      await closeSecondaryAndRestoreMain(mainHandle, secondaryHandle);
      await requireTooltipToStayClosed(
        async () => isSelectorVisible(CODEMIRROR_HOVER_SELECTOR),
        "The CodeMirror tooltip reopened without fresh pointer engagement.",
      );
      await hoverTextByGeometry(".cm-content", '"tooltip"');
      await waitFor(
        async () => isSelectorVisible(CODEMIRROR_HOVER_SELECTOR),
        { message: "The CodeMirror tooltip did not reopen after fresh pointer engagement." },
      );

      // Shared Bits UI tooltip: close on deactivation, do not reopen merely
      // because focus returns, then allow a fresh pointer entry to open it.
      await hoverTestId("action-transformations");
      await waitFor(
        async () => (await readVisibleTooltips()).length > 0,
        { message: "The shared action tooltip never opened." },
      );
      secondaryHandle = await deactivateMainWindow(mainHandle);
      expect(await readVisibleTooltips()).toEqual([]);
      await closeSecondaryAndRestoreMain(mainHandle, secondaryHandle);
      await requireTooltipToStayClosed(
        async () => isSelectorVisible(SHARED_TOOLTIP_SELECTOR),
        "The shared tooltip reopened without fresh pointer engagement.",
      );
      await hoverTestId("action-transformations");
      await waitFor(
        async () => (await readVisibleTooltips()).length > 0,
        { message: "The shared tooltip did not reopen after fresh pointer engagement." },
      );

      // Keyboard focus remains on its DOM control while the native window is
      // inactive, but that retained focus must not reopen the tooltip when the
      // window becomes active again.
      await clickTestId("theme-toggle");
      await dismissTransientOverlays();
      await browser.keys([SHIFT, TAB]);
      await waitFor(
        async () => (await readFocusedTestId()) === "action-transformations",
        { message: "Shift+Tab did not focus the transformations action." },
      );
      await waitFor(
        async () => (await readVisibleTooltips()).length > 0,
        { message: "Keyboard focus did not open the shared action tooltip." },
      );
      secondaryHandle = await deactivateMainWindow(mainHandle);
      await waitFor(
        async () => (await attributeOf("action-transformations", "data-state")) === "closed",
        { message: "The keyboard-focused trigger did not clear its open state." },
      );
      expect(await readVisibleTooltips()).toEqual([]);
      await closeSecondaryAndRestoreMain(mainHandle, secondaryHandle);
      expect(await readFocusedTestId()).toBe("action-transformations");
      await requireTooltipToStayClosed(
        async () => isSelectorVisible(SHARED_TOOLTIP_SELECTOR),
        "The keyboard-focused tooltip reopened when native focus returned.",
      );

      // CSS-generated fold-gutter tooltip.
      await hoverSelectorByGeometry(FOLD_GUTTER_SELECTOR);
      await waitFor(readFoldGutterTooltipVisible, {
        message: "The fold-gutter tooltip never became visible.",
      });
      secondaryHandle = await deactivateMainWindow(mainHandle);
      expect(await readFoldGutterTooltipVisible()).toBe(false);
      await closeSecondaryAndRestoreMain(mainHandle, secondaryHandle);
      await requireTooltipToStayClosed(
        readFoldGutterTooltipVisible,
        "The fold-gutter tooltip reopened without fresh pointer engagement.",
      );
      await hoverSelectorByGeometry(FOLD_GUTTER_SELECTOR);
      await waitFor(readFoldGutterTooltipVisible, {
        message: "The fold-gutter tooltip did not reopen after fresh pointer engagement.",
      });

    },
  );
});

import { browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import {
  clickTestId,
  clickSelector,
  dismissTransientOverlays,
  hoverSelector,
  hoverTestId,
} from "../driver/interact.js";
import { pressModShift, releaseModifiers, TAB } from "../driver/keys.js";
import {
  isSelectorVisible,
  readFoldGutterTooltipVisible,
  readFocusedTestId,
  readTooltipWindowActive,
  readVisibleTooltips,
} from "../driver/probe.js";
import {
  requireConditionForDuration,
  waitFor,
} from "../driver/wait.js";
import { openText } from "../fixtures/factories.js";
import * as editor from "../pages/editor.js";

const SHARED_TOOLTIP_SELECTOR = "[data-slot='tooltip-content']";
const CODEMIRROR_HOVER_SELECTOR = ".cm-tooltip.cm-tooltip-hover";
const FOLD_GUTTER_SELECTOR = ".cm-foldGutter [data-cm-tooltip]";
const FOLD_PLACEHOLDER_SELECTOR = ".cm-foldPlaceholder";

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

      // Shared Bits UI tooltip: close on deactivation, do not reopen merely
      // because focus returns, then allow a fresh pointer entry to open it.
      await hoverTestId("action-transformations");
      await waitFor(
        async () => (await readVisibleTooltips()).length > 0,
        { message: "The shared action tooltip never opened." },
      );
      let secondaryHandle = await deactivateMainWindow(mainHandle);
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

      await dismissTransientOverlays();
      await openText(
        "tooltip-window-lifecycle.json",
        '{\n  "tooltip": {\n    "value": 1\n  }\n}\n',
      );

      // Keyboard focus remains on its DOM control while the native window is
      // inactive, but that retained focus must not reopen the tooltip when the
      // window becomes active again.
      await clickTestId("action-copy");
      await dismissTransientOverlays();
      await browser.keys(TAB);
      await waitFor(
        async () => (await readFocusedTestId()) === "action-transformations",
        { message: "Tab did not move keyboard focus to the transformations action." },
      );
      await waitFor(
        async () => (await readVisibleTooltips()).length > 0,
        { message: "Keyboard focus did not open the shared action tooltip." },
      );
      secondaryHandle = await deactivateMainWindow(mainHandle);
      expect(await readVisibleTooltips()).toEqual([]);
      await closeSecondaryAndRestoreMain(mainHandle, secondaryHandle);
      expect(await readFocusedTestId()).toBe("action-transformations");
      await requireTooltipToStayClosed(
        async () => isSelectorVisible(SHARED_TOOLTIP_SELECTOR),
        "The keyboard-focused tooltip reopened when native focus returned.",
      );

      // CSS-generated fold-gutter tooltip.
      await hoverSelector(FOLD_GUTTER_SELECTOR);
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
      await hoverSelector(FOLD_GUTTER_SELECTOR);
      await waitFor(readFoldGutterTooltipVisible, {
        message: "The fold-gutter tooltip did not reopen after fresh pointer engagement.",
      });

      // Fold the JSON object to get a tightly bounded target owned by the
      // CodeMirror hoverTooltip extension rather than relying on text geometry.
      await dismissTransientOverlays();
      await clickSelector(FOLD_GUTTER_SELECTOR);
      await hoverSelector(FOLD_PLACEHOLDER_SELECTOR);
      await waitFor(
        async () => isSelectorVisible(CODEMIRROR_HOVER_SELECTOR),
        { message: "The CodeMirror fold tooltip never became visible." },
      );
      secondaryHandle = await deactivateMainWindow(mainHandle);
      expect(await isSelectorVisible(CODEMIRROR_HOVER_SELECTOR)).toBe(false);
      await closeSecondaryAndRestoreMain(mainHandle, secondaryHandle);
      await requireTooltipToStayClosed(
        async () => isSelectorVisible(CODEMIRROR_HOVER_SELECTOR),
        "The CodeMirror tooltip reopened without fresh pointer engagement.",
      );
      await hoverSelector(FOLD_PLACEHOLDER_SELECTOR);
      await waitFor(
        async () => isSelectorVisible(CODEMIRROR_HOVER_SELECTOR),
        { message: "The CodeMirror tooltip did not reopen after fresh pointer engagement." },
      );
    },
  );
});

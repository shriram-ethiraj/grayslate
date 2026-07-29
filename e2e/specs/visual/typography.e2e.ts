import { $, browser, expect } from "@wdio/globals";
import { TIMEOUTS } from "../../config/timeouts.js";
import { scenario } from "../../coverage/scenario.js";
import { waitFor } from "../../driver/wait.js";
import { pressMod } from "../../driver/keys.js";
import { openText } from "../../fixtures/factories.js";
import { clickTestId } from "../../pages/common.js";
import { focus as focusEditor } from "../../pages/editor.js";
import { ensureOpen as ensureSidebarOpen } from "../../pages/sidebar.js";
import { waitForLanguageMode } from "../../pages/statusBar.js";
import {
  activeFileForegroundSnapshot,
  colorChannelDelta,
  controlStyle,
  expectBorderlessSelection,
  expectOpticalIconSize,
  expectSelectedHoverSeparation,
  expectVisibleHover,
  sidebarSurfaceSnapshot,
  readColumnGap,
  readRootIsDark,
  typographySnapshot,
} from "../../driver/visualProbe.js";


/**
 * Typography, iconography, and state styling.
 *
 * This suite is deliberately non-blocking: it asserts pixel sizes, font
 * weights, and computed colours, so it fails whenever the design changes on
 * purpose, and its hover assertions depend on a real window manager. Neither
 * property should be able to gate a product change.
 */
describe("Appearance and typography", () => {
  scenario(
    "visual.typography.hierarchy",
    "loads Source Sans 3 and Commit Mono with the intended hierarchy",
    async () => {
    await openText("typography-base.txt", "baseline");
    await openText(
      "typography.py",
      "# An italic comment\ndef greet(name):\n    return f\"Hello, {name}!\"\n",
    );
    await waitForLanguageMode("python");
    await clickTestId("menu-file");
    await clickTestId("menu-settings");
    await (await $("[data-testid='settings-dialog']")).waitForDisplayed({
      timeout: TIMEOUTS.ui,
      timeoutMsg: "The settings dialog never became visible.",
    });

    const typography = await typographySnapshot();
    expect(typography.allFacesLoaded).toBe(true);
    for (const family of typography.uiFamilies) {
      expect(family).toContain("Source Sans 3");
      expect(family).not.toContain("Commit Mono");
    }
    expect(typography.uiWeights.menu).toBe("400");
    expect(typography.uiWeights.status).toBe("400");
    expect(typography.uiWeights.title).toBe("500");
    expect(typography.uiWeights.activeFile).toBe("500");
    expect(typography.uiWeights.inactiveFile).toBe("400");
    expect(typography.monoFamily).toContain("Commit Mono");
    expect(typography.boldToken.family).toContain("Commit Mono");
    expect(Number.parseInt(typography.boldToken.weight, 10)).toBeGreaterThanOrEqual(700);
    expect(typography.italicToken.family).toContain("Commit Mono");
    expect(typography.italicToken.style).toBe("italic");
    expect(typography.fontSynthesis).toContain("none");
    expect(typography.gutterLineOffset).toBeLessThanOrEqual(1);

    await browser.keys("Escape");
    },
  );

  scenario(
    "visual.icons.optical-size",
    "preserves optical sizing for search-option icon libraries",
    async () => {
    await ensureSidebarOpen();
    await expectOpticalIconSize("sidebar-search-case", 17);
    await expectOpticalIconSize("sidebar-search-word", 17);
    await expectOpticalIconSize("sidebar-search-regex", 16);

    await focusEditor();
    await pressMod("f");
    const findPanel = await $("[data-testid='find-replace-panel']");
    await findPanel.waitForDisplayed({
      timeout: TIMEOUTS.ui,
      timeoutMsg: "The find panel never became visible.",
    });
    try {
      const findInputStyle = await controlStyle("find-input");
      expect(findInputStyle.borderTopWidth).toBe("1px");
      await expectOpticalIconSize("find-opt-case", 17);
      await expectOpticalIconSize("find-opt-word", 17);
      await expectOpticalIconSize("find-opt-regex", 16);
    } finally {
      await browser.keys("Escape");
      await findPanel.waitForDisplayed({
        reverse: true,
        timeout: TIMEOUTS.ui,
        timeoutMsg: "The find panel did not close.",
      });
    }

    await focusEditor();
    await pressMod("g");
    const goToDialog = await $("[data-testid='go-to-line-dialog']");
    await goToDialog.waitForDisplayed({
      timeout: TIMEOUTS.ui,
      timeoutMsg: "The go-to-line dialog never became visible.",
    });
    try {
      const goToInputStyle = await controlStyle("go-to-line-input");
      expect(goToInputStyle.borderTopWidth).toBe("1px");
    } finally {
      await browser.keys("Escape");
      await goToDialog.waitForDisplayed({
        reverse: true,
        timeout: TIMEOUTS.ui,
        timeoutMsg: "The go-to-line dialog did not close.",
      });
    }
    },
  );

  scenario(
    "visual.states.hover-and-selected",
    "uses clear hover and borderless selected states in both themes",
    async () => {
    await ensureSidebarOpen();
    const root = await $("html");
    const initiallyDark = (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark");
    expect(await readColumnGap("sidebar-tabs")).toBe("4px");

    for (let themeIndex = 0; themeIndex < 2; themeIndex += 1) {
      await expectBorderlessSelection("sidebar-tab-unified");
      await expectBorderlessSelection("sidebar-active-file");
      await expectVisibleHover("sidebar-refresh");
      await expectSelectedHoverSeparation();

      const activeFile = await $("[data-testid='sidebar-active-file']");
      const activeFileButton = await activeFile.$("button");
      expect(await activeFileButton.getAttribute("aria-current")).toBe("true");
      const foregrounds = await activeFileForegroundSnapshot();
      expect(foregrounds.icon).toBe(foregrounds.title);

      const surfaces = await sidebarSurfaceSnapshot();
      const activeStyle = await controlStyle("sidebar-active-file");
      expect(surfaces.listBackground).not.toBe(surfaces.sidebarBackground);
      expect(colorChannelDelta(activeStyle.backgroundColor, surfaces.listBackground))
        .toBeGreaterThanOrEqual(30);

      if (themeIndex === 0) {
        await clickTestId("theme-toggle");
        await waitFor(async () => (await readRootIsDark()) !== initiallyDark, {
          message: "The theme never switched while checking both themes.",
        });
      }
    }

    const currentlyDark = (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark");
    if (currentlyDark !== initiallyDark) {
      await clickTestId("theme-toggle");
      await waitFor(async () => (await readRootIsDark()) === initiallyDark, {
        message: "The theme was not restored to how the scenario found it.",
      });
    }

    await clickTestId("menu-file");
    await clickTestId("menu-settings");
    await (await $("[data-testid='settings-dialog']")).waitForDisplayed({
      timeout: TIMEOUTS.ui,
      timeoutMsg: "The settings dialog never became visible.",
    });
    await expectBorderlessSelection("settings-pane-general");
    await browser.keys("Escape");
    },
  );

});

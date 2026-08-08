import { browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import { readComputedStyleBySelector } from "../driver/probe.js";
import { restartApp } from "../driver/session.js";
import { waitFor } from "../driver/wait.js";
import { openText } from "../fixtures/factories.js";
import { pressEscape } from "../pages/common.js";
import * as app from "../pages/app.js";
import * as editor from "../pages/editor.js";
import * as settings from "../pages/settings.js";
import * as sidebar from "../pages/sidebar.js";
import * as statusBar from "../pages/statusBar.js";
import * as titleBar from "../pages/titleBar.js";

async function renderedFontSize(): Promise<string> {
  const style = await readComputedStyleBySelector(
    "[data-testid='editor'] .cm-content",
    ["font-size"],
  );
  return style?.["font-size"] ?? "";
}

async function wordWrapState(): Promise<string | null> {
  const value = await titleBar.readMenuItemAttribute(
    "edit",
    "menu-word-wrap",
    "aria-checked",
  );
  await pressEscape();
  return value;
}

async function waitForPersistedSidebarPosition(dividerX: number): Promise<void> {
  const { width } = await browser.getWindowSize();
  let observed: string | undefined;
  await waitFor(
    async () => {
      observed = await settings.persistedValue("sidebar_width");
      const percentage = Number.parseInt(observed ?? "", 10);
      return (
        Number.isFinite(percentage) &&
        Math.abs((percentage * width) / 100 - dividerX) <= 12
      );
    },
    {
      message: () =>
        `The final sidebar position was not persisted. Last stored width: ${observed}.`,
    },
  );
}

async function restoreSidebarWidth(targetValue: string): Promise<void> {
  const target = Number.parseInt(targetValue, 10);
  if (!Number.isFinite(target)) {
    throw new Error(`Cannot restore invalid sidebar width: ${targetValue}`);
  }

  // Paneforge reports and persists an integer percentage, while WebDriver drags
  // in pixels. Correct from the persisted value so pixel rounding cannot leave
  // this cleanup one percentage point away from the original setting.
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const currentValue = await settings.persistedValue("sidebar_width");
    const current = Number.parseInt(currentValue ?? "", 10);
    if (current === target) return;
    if (!Number.isFinite(current)) {
      throw new Error(`Cannot read current sidebar width: ${currentValue}`);
    }

    const { width } = await browser.getWindowSize();
    await sidebar.resizeBy(((target - current) * width) / 100);
    await waitForPersistedSidebarPosition(await sidebar.dividerX());
  }

  throw new Error(`The original sidebar width ${target}% was not restored.`);
}

describe("Restart and persisted settings", () => {
  scenario(
    "file.restart.reopens-last",
    "reopens the last saved document after a real process restart",
    async () => {
      const target = await openText("restart-last.txt", "restart retained\n");
      await settings.waitForPersistedValue("last_active_file", target);

      await settings.open();
      await settings.setStartupBehavior("last");
      await settings.close();
      await settings.waitForPersistedValue("startup_behavior", "last");

      await restartApp({ documentPath: target, documentLength: 17 });
      await editor.waitForExactText("restart retained\n");

      await settings.open();
      await settings.setStartupBehavior("new");
      await settings.close();
      await settings.waitForPersistedValue("startup_behavior", "new");
    },
  );

  scenario(
    "file.restart.new-slate",
    "starts with a blank slate when the startup preference says new",
    async () => {
      await settings.open();
      await settings.setStartupBehavior("new");
      await settings.close();
      await settings.waitForPersistedValue("startup_behavior", "new");

      await openText("restart-new.txt", "must not reopen\n");
      await restartApp({ documentPath: "New Slate", documentLength: 0 });
      expect(await editor.text()).toBe("");
    },
  );

  scenario(
    "shell.theme.persist-across-restart",
    "keeps the selected theme across a real process restart",
    async () => {
      const originalDark = await app.isDarkTheme();
      await app.toggleTheme();
      await app.waitForTheme(!originalDark);
      await settings.waitForPersistedValue("theme", originalDark ? "light" : "dark");

      await restartApp();
      expect(await app.isDarkTheme()).toBe(!originalDark);

      await app.restoreTheme(originalDark);
      await settings.waitForPersistedValue("theme", originalDark ? "dark" : "light");
    },
  );

  scenario(
    "sidebar.open.persist-across-restart",
    "restores whether the sidebar was open after restart",
    async () => {
      // An open sidebar is the shipped default, so the settings store can
      // legitimately have no `sidebar_open` row yet. Force a state transition
      // before asserting persistence instead of treating the default UI state
      // as proof that a write occurred.
      if (await sidebar.isOpen()) {
        await sidebar.ensureClosed();
        await settings.waitForPersistedValue("sidebar_open", "false");
      }
      await sidebar.ensureOpen();
      await settings.waitForPersistedValue("sidebar_open", "true");

      await restartApp();
      await waitFor(async () => sidebar.isOpen(), {
        message: "The sidebar did not reopen from its persisted setting.",
      });

      await sidebar.ensureClosed();
      await settings.waitForPersistedValue("sidebar_open", "false");
    },
  );

  scenario(
    "sidebar.width.persist-across-restart",
    "restores a user-resized sidebar width after restart",
    async () => {
      await sidebar.ensureOpen();
      const originalX = await sidebar.dividerX();
      const originalStoredWidth =
        (await settings.persistedValue("sidebar_width")) ?? "20";

      await sidebar.resizeBy(80);
      let resizedX = originalX;
      await waitFor(
        async () => {
          resizedX = await sidebar.dividerX();
          return resizedX >= originalX + 40;
        },
        { message: "Dragging the sidebar divider did not resize the pane." },
      );
      await waitForPersistedSidebarPosition(resizedX);

      await restartApp();
      await waitFor(async () => sidebar.isOpen(), {
        message: "The sidebar did not reopen for the width persistence check.",
      });
      let restoredX = 0;
      await waitFor(
        async () => {
          restoredX = await sidebar.dividerX();
          return Math.abs(restoredX - resizedX) <= 12;
        },
        {
          message: () =>
            `The restored divider settled at ${restoredX}px; expected ${resizedX}px.`,
        },
      );

      await restoreSidebarWidth(originalStoredWidth);
      await sidebar.ensureClosed();
      await settings.waitForPersistedValue("sidebar_open", "false");
    },
  );

  scenario(
    "shell.settings.persist-across-restart",
    "keeps shell and new-document defaults after restart",
    async () => {
      const originalWrap = await wordWrapState();
      const originalFont = await renderedFontSize();
      const originalAutomaticUpdateChecks =
        (await settings.persistedValue("automatic_update_checks")) ?? "true";

      await settings.open();
      await settings.setAutomaticUpdateChecks(false);
      await settings.close();
      await settings.waitForPersistedValue("automatic_update_checks", "false");

      await titleBar.editMenu("word-wrap");
      const expectedWrap = originalWrap === "true" ? "false" : "true";
      await settings.waitForPersistedValue("word_wrap", expectedWrap);

      await titleBar.viewMenu("increase-font");
      await waitFor(async () => (await renderedFontSize()) !== originalFont, {
        message: "Increasing font size did not repaint the editor.",
      });
      const expectedFont = await renderedFontSize();
      await settings.waitForPersistedValue(
        "font_size",
        String(Number.parseInt(expectedFont, 10)),
      );

      await settings.open();
      await settings.setLineEnding("crlf");
      await settings.setCharacterEncoding("utf-8-bom");
      await settings.close();
      await settings.waitForPersistedValue("default_line_ending", "crlf");
      await settings.waitForPersistedValue("default_encoding", "utf-8-bom");

      await restartApp({ documentPath: "New Slate", documentLength: 0 });
      expect(await renderedFontSize()).toBe(expectedFont);
      expect(await wordWrapState()).toBe(expectedWrap);
      await statusBar.waitForEol("crlf");
      await statusBar.waitForEncoding("utf-8-bom");
      expect(await settings.persistedValue("automatic_update_checks")).toBe("false");
      await settings.open();
      expect(await settings.automaticUpdateChecksEnabled()).toBe(false);
      await settings.close();

      // Restore the shipped defaults for scenarios that follow in this worker.
      await titleBar.viewMenu("reset-font");
      await settings.waitForPersistedValue("font_size", "14");
      if ((await wordWrapState()) !== originalWrap) {
        await titleBar.editMenu("word-wrap");
        await settings.waitForPersistedValue("word_wrap", originalWrap ?? "false");
      }
      await settings.open();
      await settings.setLineEnding("lf");
      await settings.setCharacterEncoding("utf-8");
      await settings.close();
      await settings.waitForPersistedValue("default_line_ending", "lf");
      await settings.waitForPersistedValue("default_encoding", "utf-8");
      await settings.open();
      await settings.setAutomaticUpdateChecks(originalAutomaticUpdateChecks === "true");
      await settings.close();
      await settings.waitForPersistedValue(
        "automatic_update_checks",
        originalAutomaticUpdateChecks,
      );
    },
  );
});

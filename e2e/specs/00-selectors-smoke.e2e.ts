import { $, browser, expect } from "@wdio/globals";
import { clickTestId, ensureSidebarOpen } from "../helpers/app.js";

describe("E2E selector contract", () => {
  it("exposes the stable app-shell, editor, status, and menu hooks", async () => {
    await ensureSidebarOpen();

    for (const testId of [
      "editor",
      "title-file-name",
      "sidebar-toggle",
      "sidebar-search-input",
      "sidebar-sort-trigger",
      "sidebar-tab-unified",
      "sidebar-tab-slates",
      "sidebar-tab-local",
      "language-mode",
      "status-length",
      "status-goto-line",
      "status-indent",
      "theme-toggle",
    ]) {
      await expect(await $(`[data-testid='${testId}']`)).toExist();
    }

    await clickTestId("menu-file");
    for (const testId of ["menu-new-slate", "menu-open-file", "menu-save-as", "menu-settings"]) {
      await expect(await $(`[data-testid='${testId}']`)).toExist();
    }

    await browser.keys("Escape");
    await clickTestId("menu-edit");
    for (const testId of ["menu-undo", "menu-redo", "menu-find", "menu-replace", "menu-go-to-line"]) {
      await expect(await $(`[data-testid='${testId}']`)).toExist();
    }

    await browser.keys("Escape");
    await clickTestId("menu-view");
    for (const testId of ["menu-increase-font", "menu-decrease-font", "menu-reset-font"]) {
      await expect(await $(`[data-testid='${testId}']`)).toExist();
    }
    await browser.keys("Escape");
  });

  it("does not expose browser-native tooltips in app-owned UI", async () => {
    const titledElements = await browser.execute(() =>
      Array.from(document.querySelectorAll<HTMLElement>("[title]")).map((element) => ({
        tag: element.tagName,
        title: element.getAttribute("title"),
      })),
    );

    expect(titledElements).toEqual([]);
  });
});

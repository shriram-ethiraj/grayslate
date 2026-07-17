import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  focusEditor,
  invokeInApp,
  openExternalFixture,
  typeText,
} from "../helpers/app.js";

function isMainWindowMaximized(): Promise<boolean> {
  return invokeInApp<boolean>("plugin:window|is_maximized", { label: "main" });
}

describe("Act 11 — app shell and lifecycle", () => {
  it("opens About and exposes update actions without leaving the app", async () => {
    await clickTestId("app-help-menu");
    await expect(await $("[data-testid='menu-check-updates']")).toExist();
    await clickTestId("menu-about");
    const about = await $("[data-testid='about-dialog']");
    await about.waitForDisplayed();
    expect(await about.getText()).toContain("About");
    expect(await about.getText()).toContain("Grayslate");
    await browser.keys("Escape");
    await about.waitForDisplayed({ reverse: true });
  });

  it("maximizes and restores the native window", async () => {
    const maximize = await $("button[aria-label='Maximize']");
    await maximize.waitForClickable();
    await maximize.click();

    await browser.waitUntil(isMainWindowMaximized, {
      timeoutMsg: "the native window did not enter the maximized state",
    });

    const restore = await $("button[aria-label='Restore']");
    await restore.waitForClickable();
    await restore.click();

    await browser.waitUntil(async () => !(await isMainWindowMaximized()), {
      timeoutMsg: "the native window did not return to the restored state",
    });
    await $("button[aria-label='Maximize']").waitForDisplayed();
  });

  it("intercepts native close while a local document has unsaved changes", async () => {
    await openExternalFixture("external.txt");
    await focusEditor();
    await typeText("unsaved close guard");
    await (await $("button[aria-label='Close']")).click();

    const dialog = await $("[data-testid='unsaved-changes-dialog']");
    await dialog.waitForDisplayed();
    await clickTestId("unsaved-cancel");
    await dialog.waitForDisplayed({ reverse: true });
    await expect(await $("[data-testid='editor']")).toBeDisplayed();
  });
});

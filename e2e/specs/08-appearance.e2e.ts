import { $, browser, expect } from "@wdio/globals";
import { clickTestId, newSlate } from "../helpers/app.js";

async function storedTheme(): Promise<string | null> {
  return browser.execute(() => localStorage.getItem("theme"));
}

async function backgroundToken(): Promise<string> {
  return browser.execute(() =>
    getComputedStyle(document.documentElement).getPropertyValue("--background").trim(),
  );
}

async function chooseOption(label: string): Promise<void> {
  const option = await $(`//*[ @role='option' and normalize-space(.)='${label}' ]`);
  await option.waitForDisplayed();
  await option.click();
}

describe("Act 8 — appearance and settings", () => {
  it("toggles theme and persists the chosen value across editor views", async () => {
    const root = await $("html");
    const wasDark = (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark");
    const backgroundBefore = await backgroundToken();

    await clickTestId("theme-toggle");
    await browser.waitUntil(async () =>
      (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark") !== wasDark,
    );
    expect(await storedTheme()).toBe(wasDark ? "light" : "dark");
    expect(await backgroundToken()).not.toBe(backgroundBefore);

    await newSlate();
    expect((await root.getAttribute("class") ?? "").split(/\s+/).includes("dark")).toBe(!wasDark);
  });

  it("changes default indentation in Settings and applies it to a new slate", async () => {
    await clickTestId("menu-file");
    await clickTestId("menu-settings");
    await (await $("[data-testid='settings-dialog']")).waitForDisplayed();

    await clickTestId("settings-indent-mode");
    await chooseOption("Spaces");
    await clickTestId("settings-indent-size");
    await chooseOption("4");
    await browser.keys("Escape");

    await newSlate();
    expect(await (await $("[data-testid='status-indent']")).getText()).toContain("Spaces: 4");
  });
});

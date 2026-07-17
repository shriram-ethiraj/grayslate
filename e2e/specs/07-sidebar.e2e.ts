import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { notesRoot } from "../helpers/sandbox.js";
import {
  ENTER,
  clickTestId,
  ensureSidebarOpen,
  pressMod,
  readSidebarPaths,
  setFilterTab,
  sidebarCard,
} from "../helpers/app.js";

async function chooseSort(mode: string): Promise<void> {
  await browser.execute(() =>
    document.querySelector<HTMLElement>("[data-testid='sidebar-sort-trigger']")?.focus(),
  );
  await browser.keys(ENTER);
  await (await $(`[data-testid='sidebar-sort-${mode}']`)).waitForDisplayed();
  await clickTestId(`sidebar-sort-${mode}`);
}

async function openCardMenu(filePath: string): Promise<void> {
  const card = await sidebarCard(filePath);
  await card.waitForDisplayed();
  await card.moveTo();
  const trigger = await card.$("[data-testid='sidebar-file-options']");
  await trigger.waitForClickable();
  await trigger.click();
}

describe("Act 7 — sidebar library", () => {
  it("sorts visible files by name in both directions", async () => {
    await ensureSidebarOpen();
    await setFilterTab("unified");

    await chooseSort("name-asc");
    const ascending = (await readSidebarPaths()).map((value) => path.basename(value).toLowerCase());
    expect(ascending).toEqual([...ascending].sort());

    await chooseSort("name-desc");
    const descending = (await readSidebarPaths()).map((value) => path.basename(value).toLowerCase());
    expect(descending).toEqual([...descending].sort().reverse());
  });

  it("filters Slates and Local and marks local files in the unified view", async () => {
    await setFilterTab("slates");
    const slatePaths = await readSidebarPaths();
    expect(slatePaths.length).toBeGreaterThan(0);
    expect(slatePaths.every((value) => value.startsWith(notesRoot))).toBe(true);

    await setFilterTab("local");
    const localPaths = await readSidebarPaths();
    expect(localPaths.length).toBeGreaterThan(0);
    expect(localPaths.every((value) => !value.startsWith(notesRoot))).toBe(true);

    await setFilterTab("unified");
    await expect(await $("[data-testid='sidebar-local-badge']")).toExist();
  });

  it("searches through the Rust backend and resets search options", async () => {
    const search = await $("[data-testid='sidebar-search-input']");
    await search.setValue("config");
    await browser.waitUntil(async () => (await readSidebarPaths()).some((value) => value.endsWith("config.rs")), {
      timeout: 10_000,
      interval: 200,
    });

    await clickTestId("sidebar-search-case");
    expect(await (await $("[data-testid='sidebar-search-case']")).getAttribute("aria-pressed")).toBe("true");
    await clickTestId("sidebar-clear-search");
    expect(await search.getValue()).toBe("");
    expect(await (await $("[data-testid='sidebar-search-case']")).getAttribute("aria-pressed")).toBe("false");
  });

  it("duplicates, renames, and deletes a slate with backend-driven refreshes", async () => {
    const original = path.join(notesRoot, "config.rs");
    await setFilterTab("slates");
    await (await sidebarCard(original)).waitForDisplayed();

    await openCardMenu(original);
    await clickTestId("sidebar-action-duplicate");

    let duplicate = "";
    await browser.waitUntil(() => {
      duplicate = fs.readdirSync(notesRoot)
        .map((name) => path.join(notesRoot, name))
        .find((value) => value !== original && value.endsWith(".rs")) ?? "";
      return duplicate.length > 0;
    }, { timeout: 10_000, interval: 200 });
    await (await sidebarCard(duplicate)).waitForDisplayed();

    await openCardMenu(duplicate);
    await clickTestId("sidebar-action-rename");
    const input = await $("[data-testid='rename-input']");
    await input.waitForDisplayed();
    await input.setValue("renamed-e2e.rs");
    await clickTestId("rename-submit");

    const renamed = path.join(notesRoot, "renamed-e2e.rs");
    await browser.waitUntil(() => fs.existsSync(renamed), { timeout: 10_000, interval: 200 });
    await (await sidebarCard(renamed)).waitForDisplayed();

    await openCardMenu(renamed);
    await clickTestId("sidebar-action-delete");
    await (await $("[data-testid='delete-file-dialog']")).waitForDisplayed();
    await clickTestId("delete-confirm");
    await browser.waitUntil(() => !fs.existsSync(renamed), { timeout: 10_000, interval: 200 });
    await (await sidebarCard(renamed)).waitForExist({ reverse: true, timeout: 10_000 });
    await (await $("[data-sonner-toast]")).waitForExist({ reverse: true, timeout: 10_000 });
  });

  it("focuses Find Files and collapses and restores the sidebar", async () => {
    await pressMod("p");
    await expect(await $("[data-testid='sidebar-search-input']")).toBeFocused();
    await clickTestId("sidebar-toggle");
    await (await $("[data-testid='sidebar-tab-unified']")).waitForClickable({ reverse: true });
    await clickTestId("sidebar-toggle");
    await (await $("[data-testid='sidebar-tab-unified']")).waitForClickable();
  });
});

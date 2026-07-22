import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  newSlate,
  openExternalText,
  waitForLanguageMode,
} from "../helpers/app.js";

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

async function typographySnapshot(): Promise<{
  allFacesLoaded: boolean;
  uiFamilies: string[];
  uiWeights: {
    menu: string;
    status: string;
    title: string;
    activeFile: string;
    inactiveFile: string;
  };
  monoFamily: string;
  boldToken: { family: string; weight: string };
  italicToken: { family: string; style: string };
  fontSynthesis: string;
  gutterLineOffset: number;
}> {
  return browser.execute(async () => {
    const loadedFaces = await Promise.all([
      document.fonts.load('400 14px "Source Sans 3"'),
      document.fonts.load('italic 600 14px "Source Sans 3"'),
      document.fonts.load('700 14px "Commit Mono"'),
      document.fonts.load('italic 400 14px "Commit Mono"'),
    ]);
    await document.fonts.ready;

    const uiSelectors = [
      "[data-testid='menu-file']",
      "[data-testid='title-file-name']",
      "[data-testid='sidebar-tab-unified']",
      "[data-testid='status-length']",
      "[data-testid='settings-dialog']",
    ];
    const uiElements = uiSelectors.map((selector) =>
      document.querySelector<HTMLElement>(selector),
    );
    const monoElement = document.querySelector<HTMLElement>(
      "[data-testid='editor'] .cm-scroller",
    );
    const menu = uiElements[0];
    const title = uiElements[1];
    const status = uiElements[3];
    const activeFile = document.querySelector<HTMLElement>(
      "[data-sidebar-active='true'] [data-testid='sidebar-file-title']",
    );
    const inactiveFile = document.querySelector<HTMLElement>(
      "[data-card-path]:not([data-sidebar-active]) [data-testid='sidebar-file-title']",
    );
    const content = document.querySelector<HTMLElement>(
      "[data-testid='editor'] .cm-content",
    );
    const tokens = content ? Array.from(content.querySelectorAll<HTMLElement>("span")) : [];
    const boldToken = tokens.find((token) => {
      const weight = getComputedStyle(token).fontWeight;
      return weight === "bold" || Number.parseInt(weight, 10) >= 700;
    });
    const italicToken = tokens.find(
      (token) => getComputedStyle(token).fontStyle === "italic",
    );
    const firstLine = content?.querySelector<HTMLElement>(".cm-line");
    const firstGutterLine = Array.from(
      document.querySelectorAll<HTMLElement>(".cm-lineNumbers .cm-gutterElement"),
    ).find((element) => element.textContent?.trim() === "1");
    if (
      uiElements.some((element) => !element) ||
      !monoElement ||
      !menu ||
      !title ||
      !status ||
      !activeFile ||
      !inactiveFile ||
      !content ||
      !boldToken ||
      !italicToken ||
      !firstLine ||
      !firstGutterLine
    ) {
      throw new Error("Representative UI or CodeMirror typography element is missing.");
    }

    const boldStyle = getComputedStyle(boldToken);
    const italicStyle = getComputedStyle(italicToken);

    return {
      allFacesLoaded: loadedFaces.every((faces) => faces.length > 0),
      uiFamilies: uiElements.map((element) => getComputedStyle(element!).fontFamily),
      uiWeights: {
        menu: getComputedStyle(menu).fontWeight,
        status: getComputedStyle(status).fontWeight,
        title: getComputedStyle(title).fontWeight,
        activeFile: getComputedStyle(activeFile).fontWeight,
        inactiveFile: getComputedStyle(inactiveFile).fontWeight,
      },
      monoFamily: getComputedStyle(monoElement).fontFamily,
      boldToken: { family: boldStyle.fontFamily, weight: boldStyle.fontWeight },
      italicToken: { family: italicStyle.fontFamily, style: italicStyle.fontStyle },
      fontSynthesis: getComputedStyle(content).fontSynthesis,
      gutterLineOffset: Math.abs(
        firstLine.getBoundingClientRect().top - firstGutterLine.getBoundingClientRect().top,
      ),
    };
  });
}

describe("Act 8 — appearance and settings", () => {
  it("loads Source Sans 3 and Commit Mono with the intended hierarchy", async () => {
    await openExternalText("typography-base.txt", "baseline");
    await openExternalText(
      "typography.py",
      "# An italic comment\ndef greet(name):\n    return f\"Hello, {name}!\"\n",
    );
    await waitForLanguageMode("python");
    await clickTestId("menu-file");
    await clickTestId("menu-settings");
    await (await $("[data-testid='settings-dialog']")).waitForDisplayed();

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
  });

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

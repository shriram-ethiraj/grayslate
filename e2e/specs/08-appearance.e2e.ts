import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  ensureSidebarOpen,
  focusEditor,
  newSlate,
  openExternalText,
  pressMod,
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

interface ControlStyleSnapshot {
  backgroundColor: string;
  borderTopColor: string;
  borderTopStyle: string;
  borderTopWidth: string;
  boxShadow: string;
}

async function controlStyle(testId: string): Promise<ControlStyleSnapshot> {
  return browser.execute((id) => {
    const element = document.querySelector<HTMLElement>(`[data-testid='${id}']`);
    if (!element) throw new Error(`Control ${id} is missing.`);
    const style = getComputedStyle(element);
    return {
      backgroundColor: style.backgroundColor,
      borderTopColor: style.borderTopColor,
      borderTopStyle: style.borderTopStyle,
      borderTopWidth: style.borderTopWidth,
      boxShadow: style.boxShadow,
    };
  }, testId);
}

async function expectBorderlessSelection(testId: string): Promise<void> {
  const style = await controlStyle(testId);
  expect(style.backgroundColor).not.toBe("rgba(0, 0, 0, 0)");
  expect(style.boxShadow).not.toBe("none");
  expect(style.boxShadow).not.toContain("inset");
  expect(
    style.borderTopWidth === "0px" ||
      style.borderTopStyle === "none" ||
      style.borderTopColor === "transparent" ||
      style.borderTopColor === "rgba(0, 0, 0, 0)",
  ).toBe(true);
}

async function expectVisibleHover(testId: string): Promise<void> {
  await (await $("[data-testid='editor']")).moveTo();
  const before = await controlStyle(testId);
  await (await $(`[data-testid='${testId}']`)).moveTo();
  await browser.waitUntil(
    async () => (await controlStyle(testId)).backgroundColor !== before.backgroundColor,
    {
      timeoutMsg: `${testId} did not show a visible hover background.`,
    },
  );
}

async function expectSelectedHoverSeparation(): Promise<void> {
  await clickTestId("sidebar-tab-local");
  await (await $("[data-testid='sidebar-tab-slates']")).moveTo();
  await browser.waitUntil(
    async () => {
      const selected = await controlStyle("sidebar-tab-local");
      const hovered = await controlStyle("sidebar-tab-slates");
      return colorChannelDelta(selected.backgroundColor, hovered.backgroundColor) >= 50;
    },
    {
      timeoutMsg: "Selected and hovered sidebar tabs are not visually distinct.",
    },
  );
  await clickTestId("sidebar-tab-unified");
}

function colorChannelDelta(first: string, second: string): number {
  const channels = (color: string): number[] =>
    (color.match(/\d+(?:\.\d+)?/g) ?? []).slice(0, 3).map(Number);
  const firstChannels = channels(first);
  const secondChannels = channels(second);
  if (firstChannels.length !== 3 || secondChannels.length !== 3) {
    throw new Error(`Unable to compare computed colors '${first}' and '${second}'.`);
  }
  return firstChannels.reduce(
    (total, channel, index) => total + Math.abs(channel - secondChannels[index]),
    0,
  );
}

async function sidebarSurfaceSnapshot(): Promise<{
  listBackground: string;
  sidebarBackground: string;
}> {
  return browser.execute(() => {
    const list = document.querySelector<HTMLElement>("[data-testid='sidebar-file-list']");
    const sidebar = list?.parentElement;
    if (!list || !sidebar) {
      throw new Error("Sidebar list surface is missing.");
    }
    return {
      listBackground: getComputedStyle(list).backgroundColor,
      sidebarBackground: getComputedStyle(sidebar).backgroundColor,
    };
  });
}

async function activeFileForegroundSnapshot(): Promise<{
  icon: string;
  title: string;
}> {
  return browser.execute(() => {
    const activeFile = document.querySelector<HTMLElement>("[data-testid='sidebar-active-file']");
    const icon = activeFile?.querySelector<HTMLElement>(
      "button[aria-current='true'] > [data-variant='icon']",
    );
    const title = activeFile?.querySelector<HTMLElement>("[data-testid='sidebar-file-title']");
    if (!icon || !title) {
      throw new Error("Active file icon or title is missing.");
    }
    return {
      icon: getComputedStyle(icon).color,
      title: getComputedStyle(title).color,
    };
  });
}

async function iconDimensions(testId: string): Promise<{ width: number; height: number }> {
  return browser.execute((id) => {
    const icon = document.querySelector<SVGElement>(`[data-testid='${id}'] svg`);
    if (!icon) throw new Error(`Icon for ${id} is missing.`);
    const bounds = icon.getBoundingClientRect();
    return { width: bounds.width, height: bounds.height };
  }, testId);
}

async function expectOpticalIconSize(testId: string, expectedSize: number): Promise<void> {
  const dimensions = await iconDimensions(testId);
  expect(dimensions.width).toBeGreaterThanOrEqual(expectedSize - 0.5);
  expect(dimensions.width).toBeLessThanOrEqual(expectedSize + 0.5);
  expect(dimensions.height).toBeGreaterThanOrEqual(expectedSize - 0.5);
  expect(dimensions.height).toBeLessThanOrEqual(expectedSize + 0.5);
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

  it("preserves optical sizing for search-option icon libraries", async () => {
    await ensureSidebarOpen();
    await expectOpticalIconSize("sidebar-search-case", 17);
    await expectOpticalIconSize("sidebar-search-word", 17);
    await expectOpticalIconSize("sidebar-search-regex", 16);

    await focusEditor();
    await pressMod("f");
    const findPanel = await $("[data-testid='find-replace-panel']");
    await findPanel.waitForDisplayed();
    try {
      const findInputStyle = await controlStyle("find-input");
      expect(findInputStyle.borderTopWidth).toBe("1px");
      await expectOpticalIconSize("find-opt-case", 17);
      await expectOpticalIconSize("find-opt-word", 17);
      await expectOpticalIconSize("find-opt-regex", 16);
    } finally {
      await browser.keys("Escape");
      await findPanel.waitForDisplayed({ reverse: true });
    }

    await focusEditor();
    await pressMod("g");
    const goToDialog = await $("[data-testid='go-to-line-dialog']");
    await goToDialog.waitForDisplayed();
    try {
      const goToInputStyle = await controlStyle("go-to-line-input");
      expect(goToInputStyle.borderTopWidth).toBe("1px");
    } finally {
      await browser.keys("Escape");
      await goToDialog.waitForDisplayed({ reverse: true });
    }
  });

  it("uses clear hover and borderless selected states in both themes", async () => {
    await ensureSidebarOpen();
    const root = await $("html");
    const initiallyDark = (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark");
    const tabGap = await browser.execute(() => {
      const tabList = document.querySelector<HTMLElement>("[data-testid='sidebar-tabs']");
      if (!tabList) throw new Error("Sidebar tab list is missing.");
      return getComputedStyle(tabList).columnGap;
    });
    expect(tabGap).toBe("4px");

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
        await browser.waitUntil(async () =>
          (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark") !== initiallyDark,
        );
      }
    }

    const currentlyDark = (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark");
    if (currentlyDark !== initiallyDark) {
      await clickTestId("theme-toggle");
      await browser.waitUntil(async () =>
        (await root.getAttribute("class") ?? "").split(/\s+/).includes("dark") === initiallyDark,
      );
    }

    await clickTestId("menu-file");
    await clickTestId("menu-settings");
    await (await $("[data-testid='settings-dialog']")).waitForDisplayed();
    await expectBorderlessSelection("settings-pane-general");
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

    await clickTestId("settings-pane-editor");
    await clickTestId("settings-indent-mode");
    await chooseOption("Spaces");
    await clickTestId("settings-indent-size");
    await chooseOption("4");
    await browser.keys("Escape");

    await newSlate();
    expect(await (await $("[data-testid='status-indent']")).getText()).toContain("Spaces: 4");
  });
});

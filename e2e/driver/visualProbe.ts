import { $, browser, expect } from "@wdio/globals";
import { INTERVALS, TIMEOUTS } from "../config/timeouts.js";
import { clickTestId } from "./interact.js";

/**
 * Read-only measurement for the visual suite.
 *
 * These are computed styles, font-loading state, and element geometry — none of
 * which WebDriver exposes, and none of which any other layer can provide. They
 * live in the driver layer because that is where `browser.execute` belongs, and
 * because everything here strictly *reads*: nothing dispatches an event, sets a
 * value, or mutates the page.
 *
 * Moved out of the typography spec so that spec contains assertions rather than
 * scripting, and so the convention lint applies to it like every other spec.
 */

export async function storedTheme(): Promise<string | null> {
  return browser.execute(() => localStorage.getItem("theme"));
}

export async function backgroundToken(): Promise<string> {
  return browser.execute(() =>
    getComputedStyle(document.documentElement).getPropertyValue("--background").trim(),
  );
}

export async function chooseOption(label: string): Promise<void> {
  const option = await $(`//*[ @role='option' and normalize-space(.)='${label}' ]`);
  await option.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: `The '${label}' option never became visible.`,
  });
  await option.click();
}

export interface ControlStyleSnapshot {
  backgroundColor: string;
  borderTopColor: string;
  borderTopStyle: string;
  borderTopWidth: string;
  boxShadow: string;
}

export async function controlStyle(testId: string): Promise<ControlStyleSnapshot> {
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

export async function expectBorderlessSelection(testId: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((id) => {
        const element = document.querySelector<HTMLElement>(`[data-testid='${id}']`);
        if (!element) return false;

        const style = getComputedStyle(element);
        const isSelected = element.matches(
          "[aria-pressed='true'], [data-state='active'], [data-active='true']",
        );
        return (
          isSelected &&
          style.backgroundColor !== "rgba(0, 0, 0, 0)" &&
          style.boxShadow !== "none"
        );
      }, testId),
    {
      timeout: TIMEOUTS.ui,
      interval: INTERVALS.fast,
      timeoutMsg: `${testId} never settled into its selected visual state.`,
    },
  );

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

export async function expectVisibleHover(testId: string): Promise<void> {
  await (await $("[data-testid='editor']")).moveTo();
  const before = await controlStyle(testId);
  await (await $(`[data-testid='${testId}']`)).moveTo();
  await browser.waitUntil(
    async () => (await controlStyle(testId)).backgroundColor !== before.backgroundColor,
    {
      timeout: TIMEOUTS.ui,
      timeoutMsg: `${testId} did not show a visible hover background.`,
    },
  );
}

export async function expectSelectedHoverSeparation(): Promise<void> {
  await clickTestId("sidebar-tab-local");
  await (await $("[data-testid='sidebar-tab-slates']")).moveTo();
  await browser.waitUntil(
    async () => {
      const selected = await controlStyle("sidebar-tab-local");
      const hovered = await controlStyle("sidebar-tab-slates");
      return colorChannelDelta(selected.backgroundColor, hovered.backgroundColor) >= 50;
    },
    {
      timeout: TIMEOUTS.ui,
      timeoutMsg: "Selected and hovered sidebar tabs are not visually distinct.",
    },
  );
  await clickTestId("sidebar-tab-unified");
}

export function colorChannelDelta(first: string, second: string): number {
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

export async function sidebarSurfaceSnapshot(): Promise<{
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

export async function activeFileForegroundSnapshot(): Promise<{
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

export async function iconDimensions(testId: string): Promise<{ width: number; height: number }> {
  return browser.execute((id) => {
    const icon = document.querySelector<SVGElement>(`[data-testid='${id}'] svg`);
    if (!icon) throw new Error(`Icon for ${id} is missing.`);
    const bounds = icon.getBoundingClientRect();
    return { width: bounds.width, height: bounds.height };
  }, testId);
}

export async function expectOpticalIconSize(testId: string, expectedSize: number): Promise<void> {
  const dimensions = await iconDimensions(testId);
  expect(dimensions.width).toBeGreaterThanOrEqual(expectedSize - 0.5);
  expect(dimensions.width).toBeLessThanOrEqual(expectedSize + 0.5);
  expect(dimensions.height).toBeGreaterThanOrEqual(expectedSize - 0.5);
  expect(dimensions.height).toBeLessThanOrEqual(expectedSize + 0.5);
}

export async function typographySnapshot(): Promise<{
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

/** The computed column gap of a container, by `data-testid`. */
export async function readColumnGap(testId: string): Promise<string> {
  return browser.execute((id) => {
    const element = document.querySelector<HTMLElement>(`[data-testid='${id}']`);
    if (!element) throw new Error(`Container ${id} is missing.`);
    return getComputedStyle(element).columnGap;
  }, testId);
}

/** Whether the document root carries the dark theme class. */
export async function readRootIsDark(): Promise<boolean> {
  return browser.execute(() => document.documentElement.classList.contains("dark"));
}

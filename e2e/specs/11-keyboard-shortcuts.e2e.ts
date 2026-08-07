import { $$, expect } from "@wdio/globals";
import { formatForDisplay } from "@tanstack/hotkeys";
import { scenario } from "../coverage/scenario.js";
import { hoverTestId } from "../driver/interact.js";
import { readVisibleTooltips } from "../driver/probe.js";
import { waitFor } from "../driver/wait.js";
import { pressEscape } from "../pages/common.js";
import * as dialogs from "../pages/dialogs.js";
import * as titleBar from "../pages/titleBar.js";

/**
 * Shortcut discoverability.
 *
 * Shortcuts are formatted through the app's own `@tanstack/hotkeys`
 * `formatForDisplay`, so the expectations here are computed the same way the UI
 * computes them rather than hard-coded to one platform's modifier. The suite
 * previously asserted a literal `Ctrl+N`, which would fail on macOS.
 */
const displayPlatform =
  process.platform === "darwin" ? "mac" : process.platform === "win32" ? "windows" : "linux";

const shortcut = (key: string): string =>
  formatForDisplay(key, { platform: displayPlatform });

async function expectTooltip(testId: string, expected: string): Promise<void> {
  await hoverTestId(testId);
  let seen: string[] = [];
  await waitFor(
    async () => {
      seen = await readVisibleTooltips();
      return seen.includes(expected);
    },
    {
      message: () => `Tooltip for '${testId}' never showed ${JSON.stringify(expected)}. Saw: ${JSON.stringify(seen)}`,
    },
  );
}

describe("Keyboard shortcuts help", () => {
  scenario(
    "shell.shortcuts.tooltips",
    "shows primary platform shortcuts in actionable tooltips",
    async () => {
      await expectTooltip("action-transformations", `Open transformations (${shortcut("Mod+K")})`);
      await expectTooltip("status-goto-line", `Go to line (${shortcut("Mod+G")})`);
    },
  );

  scenario(
    "shell.shortcuts.dialog",
    "opens from Help, lists every section, and filters as a plain list",
    async () => {
      await titleBar.helpMenu("keyboard-shortcuts");
      await dialogs.keyboardShortcuts.waitForOpen();

      const listed = await dialogs.keyboardShortcuts.listText();
      for (const label of ["New Slate", "Save", "Go To Line", "Select All"]) {
        expect(listed).toContain(label);
      }

      // The list is reference material, not a picker: it must expose no
      // selection semantics that would imply the rows are actionable.
      const rows = await $$("[data-testid='keyboard-shortcuts-list'] li");
      expect(rows.length).toBeGreaterThan(0);
      for (const row of [...rows].slice(0, 10)) {
        expect(await row.getAttribute("aria-selected")).toBeNull();
        expect(await row.getAttribute("role")).not.toBe("option");
      }

      // Search must both include matches and exclude non-matches.
      await dialogs.keyboardShortcuts.search("slate");
      await waitFor(
        async () => {
          const text = await dialogs.keyboardShortcuts.listText();
          return text.includes("New Slate") && !text.includes("Go To Line");
        },
        { message: "Searching for 'slate' did not filter the shortcut list." },
      );

      await dialogs.keyboardShortcuts.search("zzzznotashortcut");
      await waitFor(
        async () => (await dialogs.keyboardShortcuts.listText()).trim().length < 80,
        { message: "A query with no matches did not collapse the shortcut list." },
      );

      await pressEscape();
    },
  );
});

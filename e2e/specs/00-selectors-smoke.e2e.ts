import { expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import { countElements } from "../driver/probe.js";
import { existsTestId, pressEscape } from "../pages/common.js";
import * as titleBar from "../pages/titleBar.js";

/**
 * A fast smoke test over the app's stable test hooks.
 *
 * This exists to fail *first* and *cheaply*. A renamed or removed `data-testid`
 * would otherwise surface as a dozen confusing timeouts spread across later
 * specs; here it fails in seconds with the missing hook named. It deliberately
 * asserts presence only — the behavior behind each hook is covered by the spec
 * that owns it.
 */
const SHELL_HOOKS = [
  "editor",
  "sidebar-toggle",
  "theme-toggle",
  "header-new-slate",
  "title-file-name",
  "status-length",
  "status-goto-line",
  "status-indent",
  "status-eol",
  "status-encoding",
  "language-mode",
  "action-copy",
  "action-transformations",
  "window-minimize",
  "window-maximize",
  "window-close",
];

const MENU_HOOKS: [Parameters<typeof titleBar.readMenuItemAttribute>[0], string[]][] = [
  ["file", ["menu-new-slate", "menu-open-file", "menu-save", "menu-save-as", "menu-settings"]],
  ["edit", ["menu-undo", "menu-redo", "menu-find", "menu-word-wrap", "menu-select-all"]],
  ["view", ["menu-increase-font", "menu-decrease-font", "menu-reset-font"]],
  ["help", ["help-keyboard-shortcuts", "menu-check-updates", "menu-about"]],
];

describe("Selectors smoke", () => {
  scenario(
    "harness.test-hooks-present",
    "exposes every stable app-shell, editor, status, and menu hook",
    async () => {
      const missingShell: string[] = [];
      for (const hook of SHELL_HOOKS) {
        if (!(await existsTestId(hook))) missingShell.push(hook);
      }
      expect(missingShell).toEqual([]);

      const missingMenu: string[] = [];
      for (const [menu, hooks] of MENU_HOOKS) {
        // Opening the menu is what mounts its items.
        await titleBar.readMenuItemAttribute(menu, hooks[0]!, "data-testid");
        for (const hook of hooks) {
          if (!(await existsTestId(hook))) missingMenu.push(`${menu}:${hook}`);
        }
        await pressEscape();
      }
      expect(missingMenu).toEqual([]);
    },
  );

  scenario(
    "harness.no-native-tooltips",
    "uses the app's own tooltips rather than browser-native title attributes",
    async () => {
      // A stray `title` attribute produces an OS tooltip that sits above the
      // app's own, and — as this suite learned the hard way — intercepts clicks
      // in ways that look like product bugs.
      expect(await countElements("[title]")).toBe(0);
    },
  );
});

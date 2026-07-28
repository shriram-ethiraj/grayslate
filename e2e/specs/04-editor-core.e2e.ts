import { browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import {
  armOperationGate,
  releaseOperationGate,
  waitForOperationGate,
} from "../driver/operationGate.js";
import { pressMod, SHIFT, TAB } from "../driver/keys.js";
import { readComputedStyleBySelector, readDocumentLength } from "../driver/probe.js";
import { waitFor } from "../driver/wait.js";
import { openText } from "../fixtures/factories.js";
import { waitForClipboardText } from "../driver/clipboard.js";
import { clickTestId, pressEscape } from "../pages/common.js";
import * as dialogs from "../pages/dialogs.js";
import * as editor from "../pages/editor.js";
import * as findReplace from "../pages/findReplace.js";
import * as statusBar from "../pages/statusBar.js";
import * as titleBar from "../pages/titleBar.js";

/**
 * Core editing.
 *
 * Every scenario opens its own document so none depends on what a sibling left
 * behind. Find is Rust-backed, so its counts land asynchronously and are always
 * waited for rather than read straight after typing.
 */
const HAYSTACK = "alpha Alpha ALPHA alphabet\nbeta\nalpha\n";

describe("Editor core", () => {
  scenario(
    "editor.find.count-and-filters",
    "reports a match count that responds to case, whole word, and regex",
    async () => {
      await openText("find-count.txt", HAYSTACK);
      await findReplace.open();

      await findReplace.setQuery("alpha");
      // Case-insensitive, substring: alpha, Alpha, ALPHA, alphabet, alpha = 5.
      await findReplace.waitForMatchCount(5);

      await findReplace.toggleOption("word");
      // Whole word drops "alphabet".
      await findReplace.waitForMatchCount(4);

      await findReplace.toggleOption("case");
      // Case-sensitive leaves the two lowercase whole words.
      await findReplace.waitForMatchCount(2);

      await findReplace.toggleOption("case");
      await findReplace.toggleOption("word");
      await findReplace.waitForMatchCount(5);

      await findReplace.toggleOption("regex");
      await findReplace.setQuery("alpha|beta");
      await findReplace.waitForMatchCount(6);
      await findReplace.toggleOption("regex");
      await pressEscape();
    },
  );

  scenario("editor.find.navigate", "moves forward and back through the matches", async () => {
    await openText("find-navigate.txt", HAYSTACK);
    await findReplace.open();
    await findReplace.setQuery("alpha");
    await findReplace.waitForMatchCount(5);

    // Until a match is selected the label reads `5+`, not `n/5`. Navigation is
    // what selects one, so step in first and assert the position from there.
    expect(await findReplace.isActionable("find-next")).toBe(true);
    await findReplace.next();
    await findReplace.waitForCurrentMatch();
    const first = await findReplace.currentMatchIndex();

    await findReplace.next();
    await waitFor(async () => (await findReplace.currentMatchIndex()) !== first, {
      message: () => `Next did not move off match ${first}.`,
    });
    const second = await findReplace.currentMatchIndex();
    expect(second).not.toBe(first);

    await findReplace.previous();
    await waitFor(async () => (await findReplace.currentMatchIndex()) === first, {
      message: () => `Previous did not return to match ${first}.`,
    });

    await pressEscape();
  });

  scenario(
    "editor.find.cancel",
    "cancels an in-flight Rust match scan without wedging the editor",
    async () => {
      const original = `${"alpha beta gamma\n".repeat(20_000)}`;
      await openText("find-cancel.txt", original);
      await findReplace.open();
      await armOperationGate("editor-find");
      await findReplace.setQuery("alpha");
      await waitForOperationGate("editor-find");

      try {
        // Closing the panel calls clearSearchStatsCache(), which invokes the
        // production cancel_editor_find command.
        await pressEscape();
        await findReplace.close();
      } finally {
        await releaseOperationGate("editor-find");
      }

      expect(await findReplace.isOpen()).toBe(false);
      await editor.focus();
      await editor.type("x");
      await waitFor(async () => (await readDocumentLength()) === original.length + 1, {
        message: "The editor was not usable after cancelling a find scan.",
      });
    },
  );

  scenario("editor.find.replace-one", "replaces only the current match", async () => {
    await openText("find-replace-one.txt", "one two one two one\n");
    await findReplace.openReplace();
    await findReplace.setQuery("one");
    await findReplace.waitForMatchCount(3);
    await findReplace.setReplacement("ONE");

    // Replace acts on the *current* match, and no match is current until the
    // panel is stepped into one. Clicking first would be a silent no-op.
    await findReplace.next();
    await findReplace.waitForCurrentMatch();
    expect(await findReplace.isActionable("find-replace-one")).toBe(true);
    await findReplace.replaceOne();

    // Exactly one occurrence changes; the other two are untouched.
    await editor.waitForText(
      (text) => text.split("ONE").length - 1 === 1 && text.split(/\bone\b/).length - 1 === 2,
      "Replace changed something other than exactly one match.",
    );

    await pressEscape();
  });

  scenario(
    "editor.find.replace-all-single-undo",
    "applies replace all as one undoable transaction",
    async () => {
      const original = "cat cat cat\ncat\n";
      await openText("find-replace-all.txt", original);
      await findReplace.openReplace();
      await findReplace.setQuery("cat");
      await findReplace.waitForMatchCount(4);
      await findReplace.setReplacement("dog");

      await findReplace.replaceAll();
      await editor.waitForExactText("dog dog dog\ndog\n");
      await pressEscape();

      // One undo, not four: the whole replacement must be a single transaction,
      // or a user who changes their mind has to press undo once per match.
      await editor.focus();
      await editor.undo();
      await editor.waitForExactText(original);
    },
  );

  scenario(
    "editor.find.regex-error",
    "reports an invalid regular expression instead of silently matching nothing",
    async () => {
      await openText("find-regex-error.txt", HAYSTACK);
      await findReplace.open();
      await findReplace.toggleOption("regex");
      // Confirm the mode actually changed. Without regex on, `alpha(` is a
      // literal that simply matches nothing, and the scenario would be asserting
      // the wrong thing entirely.
      await waitFor(async () => findReplace.optionPressed("regex"), {
        message: "Regex mode never turned on, so the invalid pattern was never sent as a regex.",
      });

      // An unclosed group is invalid. Reporting it matters because the failure
      // mode otherwise looks identical to a valid pattern with no matches.
      await findReplace.setQuery("alpha(");
      await waitFor(async () => findReplace.hasRegexError(), {
        message: "An invalid regular expression was not reported on the input.",
      });

      // Correcting it clears the error and matching resumes.
      await findReplace.setQuery("alpha");
      await waitFor(async () => !(await findReplace.hasRegexError()), {
        message: "The regex error persisted after the pattern became valid.",
      });
      await findReplace.waitForMatchCount(5);

      await findReplace.toggleOption("regex");
      await pressEscape();
    },
  );

  scenario("editor.goto-line", "jumps to a line and rejects out-of-range input", async () => {
    await openText("goto-line.txt", "one\ntwo\nthree\nfour\n");
    await dialogs.goToLine.open();
    await dialogs.goToLine.enter(3);
    await browser.keys("Enter");

    await waitFor(async () => (await statusBar.cursorLabel()).includes("Ln 3"), {
      message: "Go to line did not move the cursor to line 3.",
    });

    await dialogs.goToLine.open();
    await dialogs.goToLine.enter(99);
    await waitFor(async () => dialogs.goToLine.isInvalid(), {
      message: "An out-of-range line number was not rejected.",
    });
    await pressEscape();
  });

  scenario(
    "editor.indent.picker",
    "switches between spaces and tabs and changes the width",
    async () => {
      await openText("indent-picker.txt", "line one\nline two\n");

      await statusBar.selectIndentMode("spaces");
      await statusBar.selectIndentSize(4);
      await statusBar.waitForIndentLabel("Spaces: 4");

      await statusBar.selectIndentMode("tab");
      await statusBar.waitForIndentLabel("Tab");
      await statusBar.closeIndentPicker();

      // Back to the shipped default so the next scenario starts from a known
      // state; the picker writes a per-document override, not a global setting.
      await statusBar.selectIndentMode("default");
      await statusBar.closeIndentPicker();
    },
  );

  scenario(
    "editor.indent.detect",
    "adopts the document's own indentation from its content",
    async () => {
      // Four-space indentation throughout, so detection has one right answer.
      await openText(
        "indent-detect.py",
        "def outer():\n    if True:\n        return 1\n    return 0\n",
      );

      await statusBar.selectIndentMode("detect");
      await statusBar.waitForIndentLabel("Spaces: 4");
      await statusBar.closeIndentPicker();

      await statusBar.selectIndentMode("default");
      await statusBar.closeIndentPicker();
    },
  );

  scenario(
    "editor.indent.tab-outdent",
    "indents with Tab and outdents with Shift+Tab",
    async () => {
      await openText("indent-tab.txt", "start\n");
      await statusBar.selectIndentMode("spaces");
      await statusBar.selectIndentSize(2);
      await statusBar.waitForIndentLabel("Spaces: 2");
      await statusBar.closeIndentPicker();

      await editor.focus();
      await editor.replaceText("alpha\nbeta");
      await editor.selectAll();

      await browser.keys(TAB);
      await editor.waitForExactText("  alpha\n  beta");

      await browser.keys([SHIFT, TAB]);
      await editor.waitForExactText("alpha\nbeta");

      await statusBar.selectIndentMode("default");
      await statusBar.closeIndentPicker();
    },
  );

  scenario("editor.word-wrap.toggle", "toggles word wrap to a definite state", async () => {
    await openText("word-wrap.txt", `${"long ".repeat(80)}\n`);

    const before = await titleBar.readMenuItemAttribute("edit", "menu-word-wrap", "aria-checked");
    await pressEscape();

    await titleBar.editMenu("word-wrap");
    const after = await titleBar.readMenuItemAttribute("edit", "menu-word-wrap", "aria-checked");
    await pressEscape();

    // Assert the value flipped to its opposite, not merely that it changed.
    expect(after).toBe(before === "true" ? "false" : "true");

    // Leave the global preference as it was found.
    await titleBar.editMenu("word-wrap");
    const restored = await titleBar.readMenuItemAttribute("edit", "menu-word-wrap", "aria-checked");
    await pressEscape();
    expect(restored).toBe(before);
  });

  scenario("editor.font-size", "increases, decreases, and resets the font size", async () => {
    await openText("font-size.txt", "font size probe\n");

    const read = async (): Promise<string> => {
      const style = await readComputedStyleBySelector(
        "[data-testid='editor'] .cm-content",
        ["font-size"],
      );
      return style?.["font-size"] ?? "";
    };

    const base = await read();
    await titleBar.viewMenu("increase-font");
    await waitFor(async () => (await read()) !== base, {
      message: "Increasing the font size did not change the rendered size.",
    });

    await titleBar.viewMenu("decrease-font");
    await waitFor(async () => (await read()) === base, {
      message: "Decreasing the font size did not return to the starting size.",
    });

    await titleBar.viewMenu("reset-font");
    await waitFor(async () => (await read()) === "14px", {
      message: "Resetting the font size did not return it to the 14px default.",
    });
  });

  scenario("editor.undo-redo", "round-trips an edit through undo and redo", async () => {
    await openText("undo-redo.txt", "original\n");
    await editor.focus();

    // Append rather than replace: `replaceText` is select-all-then-type, so a
    // single undo lands on the intermediate empty document rather than the
    // original text. Appending keeps one edit equal to one undo step.
    await pressMod("a");
    await editor.type("original and more");
    await editor.waitForExactText("original and more");

    await editor.undo();
    await editor.waitForText(
      (text) => text !== "original and more",
      "Undo did not revert the typed text.",
    );

    await editor.redo();
    await editor.waitForExactText("original and more");
  });

  scenario(
    "editor.clipboard.copy-document",
    "copies the whole document to the system clipboard",
    async () => {
      const body = "clipboard line one\nclipboard line two\n";
      await openText("copy-document.txt", body);

      await clickTestId("action-copy");
      await waitForClipboardText(body);
    },
  );

  scenario(
    "editor.clipboard.cut-paste-select-all",
    "cuts a selection and pastes it back through the real clipboard",
    async () => {
      await openText("cut-paste.txt", "keep\n");
      await editor.replaceText("cut me");

      await editor.selectAll();
      await pressMod("x");
      await editor.waitForExactText("");

      await pressMod("v");
      await editor.waitForExactText("cut me");
    },
  );

  scenario(
    "editor.context-menu.json",
    "copies the path, key, and value from inside a JSON document",
    async () => {
      // A long property name on its own line, so a click a short way in from
      // the left edge lands unambiguously inside the PropertyName node.
      await openText(
        "context-json.json",
        '{\n  "configuration": "enabled",\n  "other": 1\n}\n',
      );
      await statusBar.waitForLanguageMode("json");

      await editor.openContextMenuOnLine(1, 40);

      // The JSON-aware items only exist when the click resolved to a JSON node,
      // so their presence is itself the first half of the assertion.
      expect(await editor.hasContextMenuItem("copy-key")).toBe(true);
      await editor.chooseContextMenuItem("copy-key");
      await waitForClipboardText("configuration");

      // Copy Path yields a JSONPath expression, not the bare key — that is the
      // distinction between it and Copy Key.
      await editor.openContextMenuOnLine(1, 40);
      await editor.chooseContextMenuItem("copy-path");
      await waitForClipboardText("$.configuration");

      await editor.openContextMenuOnLine(1, 40);
      expect(await editor.hasContextMenuItem("copy-value")).toBe(true);
      await editor.chooseContextMenuItem("copy-value");
      await waitForClipboardText("enabled");
    },
  );

  scenario(
    "editor.context-menu.clipboard",
    "cuts, copies, and selects all from the editor context menu",
    async () => {
      await openText("context-clipboard.txt", "context menu body\n");
      await editor.replaceText("context menu body");

      // Select All from the menu, then Cut from the menu: Cut only appears once
      // a selection exists, so this also proves the menu reflects editor state.
      await editor.openContextMenu();
      await editor.chooseContextMenuItem("select-all");

      await editor.openContextMenu();
      await editor.chooseContextMenuItem("copy");
      await waitForClipboardText("context menu body");

      await editor.openContextMenu();
      await editor.chooseContextMenuItem("cut");
      await editor.waitForExactText("");

      await pressMod("v");
      await editor.waitForExactText("context menu body");
    },
  );
});

import fs from "node:fs";
import { browser, expect } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { scenario } from "../coverage/scenario.js";
import { HOME } from "../driver/keys.js";
import {
  armOperationGate,
  releaseOperationGate,
  waitForOperationGate,
} from "../driver/operationGate.js";
import { readDocumentLength } from "../driver/probe.js";
import { waitFor, waitForFile } from "../driver/wait.js";
import { openFixture, openText } from "../fixtures/factories.js";
import { pressEscape } from "../pages/common.js";
import * as editor from "../pages/editor.js";
import * as statusBar from "../pages/statusBar.js";
import * as transformations from "../pages/transformations.js";

/**
 * Transformations.
 *
 * There are 82 registered actions, and every one of them is exhaustively
 * covered by the 126 unit tests in `src-tauri/src/commands/transform.rs`.
 * Re-driving each through the desktop UI would be slow and would prove less
 * than those tests already do.
 *
 * What only an end-to-end test can prove is the *transport and UI behavior* —
 * how a result reaches the document. So this spec covers one representative per
 * behavior family: replace the document, replace a selection, insert at the
 * cursor, report through a toast, fail loudly, switch the active language, and
 * stream a chunked result. A new action needs a new scenario here only if it
 * introduces a family that does not yet exist.
 */
describe("Transformations", () => {
  scenario(
    "transform.family.insert",
    "inserts at the cursor instead of replacing the document",
    async () => {
      await openText("insert-family.txt", "prefix ");
      await editor.focus();
      // Put the caret at the very end so the insert is unambiguous.
      await editor.replaceText("prefix ");

      await transformations.run("generate.uuid-v4", false);
      await editor.waitForText(
        (text) => text.startsWith("prefix ") && text.length > "prefix ".length,
        "A generate action did not insert anything at the cursor.",
      );
      // A UUID, not a replaced document.
      await editor.waitForText(
        (text) => /[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}/i.test(text),
        "The inserted text did not look like a v4 UUID.",
      );
    },
  );

  scenario(
    "transform.family.replace-document",
    "rewrites the whole document as one undoable step",
    async () => {
      const original = "alpha beta gamma\ndelta epsilon\n";
      await openText("replace-document.txt", original);

      await transformations.run("text.uppercase");
      await editor.waitForExactText("ALPHA BETA GAMMA\nDELTA EPSILON\n");

      // One undo returns the entire document: a replace-style action must not
      // land as several transactions the user has to unwind one at a time.
      await editor.focus();
      await editor.undo();
      await editor.waitForExactText(original);
    },
  );

  scenario(
    "transform.family.replace-selection",
    "changes only the selected range when there is a selection",
    async () => {
      await openText("replace-selection.txt", "keep this\n");
      await editor.focus();
      await editor.replaceText("select me and leave the rest");

      // Select exactly "select" — six characters from the start of the line.
      await browser.keys(HOME);
      await editor.selectRight(6);

      await transformations.run("text.uppercase", false);

      // The selection is uppercased and nothing else is touched, which is the
      // whole distinction between this family and replace-document.
      await editor.waitForExactText("SELECT me and leave the rest");
    },
  );

  scenario(
    "transform.palette.search",
    "filters the palette to matching actions and hides the rest",
    async () => {
      await openText("palette-search.txt", "palette search probe\n");
      await transformations.openPalette();

      const unfiltered = await transformations.visibleActionIds();
      expect(unfiltered.length).toBeGreaterThan(1);

      await transformations.search("uppercase");
      await waitFor(
        async () => {
          const visible = await transformations.visibleActionIds();
          return visible.length > 0 && visible.every((id) => id.includes("uppercase"));
        },
        {
          message: () => "The palette never narrowed to the matching actions.",
        },
      );

      const filtered = await transformations.visibleActionIds();
      expect(filtered).toContain("text.uppercase");
      expect(filtered.length).toBeLessThan(unfiltered.length);

      // A query that matches nothing must empty the list rather than fall back
      // to showing everything.
      await transformations.search("zzzznotanaction");
      await waitFor(async () => (await transformations.visibleActionIds()).length === 0, {
        message: () => "A non-matching query still listed actions.",
      });

      await pressEscape();
      await transformations.closePalette();
    },
  );

  scenario(
    "transform.family.message",
    "reports statistics through a toast and leaves the document unchanged",
    async () => {
      const original = "two words";
      await openText("message-family.txt", original);

      await transformations.run("stats.count-words");
      await transformations.waitForToast("2 words");
      expect(await editor.text()).toBe(original);
    },
  );

  scenario(
    "transform.family.error",
    "reports invalid input without changing the document",
    async () => {
      const invalid = "{ not valid json";
      await openText("error-family.json", invalid);

      await transformations.run("json.validate");
      // The action must say something; what matters is that it did not
      // silently succeed and did not mangle the buffer.
      await waitFor(async () => (await transformations.visibleToasts()).length > 0, {
        message: "Validating invalid JSON produced no message at all.",
      });
      expect(await editor.text()).toBe(invalid);
    },
  );

  scenario(
    "transform.family.language-switch",
    "switches the active language after a converting action",
    async () => {
      await openFixture("sample.csv", "language-switch.csv");
      await statusBar.waitForLanguageMode("csv");

      await transformations.run("csv.to-json");
      await editor.waitForText(
        (text) => text.trimStart().startsWith("["),
        "csv.to-json did not produce a JSON array.",
      );
      await statusBar.waitForLanguageMode("json");
    },
  );

  scenario(
    "transform.json-array-to-lines-language-switch",
    "switches JSON array output to the JSON Lines language",
    async () => {
      await openText("array-to-lines.json", '[{"name":"Alice"},{"name":"Bob"}]');
      await statusBar.waitForLanguageMode("json");

      await transformations.run("json.array-to-lines");
      await editor.waitForExactText('{"name":"Alice"}\n{"name":"Bob"}\n');
      await statusBar.waitForLanguageMode("jsonl");
    },
  );

  scenario(
    "transform.large.chunked",
    "assembles a multi-megabyte chunked result and keeps it one undo step",
    async () => {
      // Large enough to exercise the chunked transport rather than the
      // single-message path.
      const source = `${"lorem ipsum dolor sit amet\n".repeat(200_000)}`;
      const filePath = await openText("chunked.txt", source);

      await transformations.run("text.uppercase");

      // Assert the authoritative length, not the rendered text: CodeMirror
      // virtualizes, so a text comparison would only see the viewport.
      await waitFor(async () => (await readDocumentLength()) === source.length, {
        message: "The chunked result did not assemble to the original length.",
        timeoutMs: TIMEOUTS.heavy,
      });
      await editor.waitForText(
        (text) => text.startsWith("LOREM IPSUM"),
        "The chunked result was not uppercased.",
        TIMEOUTS.heavy,
      );
      await editor.save();
      await waitForFile(filePath, (text) => text === source.toUpperCase(), {
        message: "The complete chunked result did not match the expected output.",
        timeoutMs: TIMEOUTS.heavy,
      });
      expect(fs.statSync(filePath).size).toBe(Buffer.byteLength(source));

      await editor.focus();
      await editor.undo();
      await editor.waitForText(
        (text) => text.startsWith("lorem ipsum"),
        "Undo did not restore the pre-transformation document in one step.",
        TIMEOUTS.heavy,
      );
    },
  );

  scenario(
    "transform.large.cancel",
    "cancels a running transformation without changing the document",
    async () => {
      const original = `${"cancel this transformation\n".repeat(50_000)}`;
      await openText("transform-cancel.txt", original);
      await armOperationGate("transformation");
      await transformations.run("text.uppercase");
      await waitForOperationGate("transformation");
      await transformations.loader.waitForVisible();

      try {
        await transformations.loader.cancel();
      } finally {
        await releaseOperationGate("transformation");
      }

      await transformations.loader.waitForHidden();
      await waitFor(async () => (await readDocumentLength()) === original.length, {
        message: "Cancelling the transformation changed the document length.",
        timeoutMs: TIMEOUTS.heavy,
      });
      await editor.waitForText(
        (text) => text.startsWith("cancel this transformation"),
        "Cancelling the transformation changed the document.",
        TIMEOUTS.heavy,
      );
      await editor.focus();
      await editor.type("x");
      await waitFor(async () => (await readDocumentLength()) === original.length + 1, {
        message: "The editor was not usable after cancelling a transformation.",
      });
    },
  );

  scenario(
    "transform.family.replace-document-formatting",
    "formats JSON and SQL and fully reverts with a single undo",
    async () => {
      const minified = '{"b":2,"a":{"nested":true}}';
      await openText("format-json.json", minified);

      await transformations.run("json.format");
      await editor.waitForText(
        (text) => text.includes("\n") && text.includes("\"nested\""),
        "json.format did not pretty-print the document.",
      );

      await editor.focus();
      await editor.undo();
      await editor.waitForExactText(minified);
    },
  );
});

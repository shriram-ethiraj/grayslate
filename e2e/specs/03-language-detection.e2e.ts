import { expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import { openFixture, openText } from "../fixtures/factories.js";
import * as statusBar from "../pages/statusBar.js";

/**
 * Language recognition, as the user sees it.
 *
 * The exhaustive per-language matrix lives in the `crates/grayslate-langdetect`
 * unit tests — driving 40-odd languages through the desktop UI would be far
 * slower and would prove less about the detector itself.
 *
 * What only an end-to-end test can show is that detection actually reaches the
 * status bar: that opening a real file of each major *family* lights up the
 * right language in the UI. The previous version called `detect_language` over
 * IPC in a loop, which exercised no UI at all and left the committed fixtures
 * unopened.
 */
const FAMILIES: { fixture: string; as: string; language: string }[] = [
  { fixture: "sample.py", as: "detect-python.py", language: "python" },
  { fixture: "sample.json", as: "detect-json.json", language: "json" },
  { fixture: "sample.jsonl", as: "detect-json-lines.jsonl", language: "jsonl" },
  { fixture: "sample.sql", as: "detect-sql.sql", language: "sql" },
  { fixture: "sample.md", as: "detect-markdown.md", language: "markdown" },
  { fixture: "sample.csv", as: "detect-csv.csv", language: "csv" },
];

describe("Language recognition", () => {
  scenario(
    "language.detect-from-file",
    "shows the right language in the status bar for each major file family",
    async () => {
      for (const family of FAMILIES) {
        await openFixture(family.fixture, family.as);
        // The mode is what the rest of the app keys off — highlighting, the
        // transformation suggestions, the canonical save extension.
        await statusBar.waitForLanguageMode(family.language);
      }
    },
  );

  scenario(
    "language.manual-override",
    "overrides the detected language and returns to automatic detection",
    async () => {
      await openText("override.py", "import sys\nprint(sys.argv)\n");
      await statusBar.waitForDetectedLanguage("python");

      // An explicit choice must win over detection.
      await statusBar.selectLanguage("json");
      await statusBar.waitForLanguageMode("json");

      // Returning to Auto must restore detection rather than latch the choice.
      await statusBar.selectLanguage("auto");
      await statusBar.waitForLanguageMode("auto");
      await statusBar.waitForDetectedLanguage("python");
      expect(await statusBar.detectedLanguage()).toBe("python");
    },
  );
});

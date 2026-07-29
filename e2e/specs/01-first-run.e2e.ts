import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { directoryInventory } from "../assertions/matchers.js";
import { scenario } from "../coverage/scenario.js";
import { waitFor } from "../driver/wait.js";
import { notesRoot } from "../fixtures/factories.js";
import * as app from "../pages/app.js";
import * as editor from "../pages/editor.js";
import * as sidebar from "../pages/sidebar.js";
import * as statusBar from "../pages/statusBar.js";

/**
 * First run: type, detect, autosave, name, and reopen.
 *
 * This is the flow a brand-new user meets first, and it spans the whole stack —
 * content detection in Rust, the naming registry, the autosave timer, the
 * library, and the reopen path.
 */
const RUST_SOURCE = `use std::collections::HashMap;
#[derive(Debug, Clone)]
pub struct Config { pub name: String }
pub fn process(config: &Config) -> Result<(), String> { println!("Processing: {}", config.name); Ok(()) }
`;

const SQL_SOURCE = `SELECT id, name FROM customers WHERE active = 1 ORDER BY name;
`;

/** Wait for the single autosaved file whose content matches, and return it. */
async function waitForAutosavedFile(content: string): Promise<string> {
  let found = "";
  await waitFor(
    () => {
      const matches = directoryInventory(notesRoot)
        .map((name) => path.join(notesRoot, name))
        .filter((candidate) => fs.readFileSync(candidate, "utf8") === content);
      if (matches.length !== 1) return false;
      found = matches[0] ?? "";
      return found !== "";
    },
    {
      message: "Autosave did not produce exactly one file with the typed content.",
      timeoutMs: TIMEOUTS.disk,
    },
  );
  return found;
}

describe("First run", () => {
  scenario(
    "file.first-run.blank-slate",
    "starts on an empty untitled slate without writing a file",
    async () => {
      // This is deliberately the first scenario in a fresh per-spec app
      // session. Calling New Slate here would erase the startup behavior the
      // requirement is meant to prove.
      const ready = await editor.waitUntilReady({
        documentPath: "New Slate",
        documentLength: 0,
      });
      expect(ready.documentPath).toBe("New Slate");
      expect(ready.documentLength).toBe(0);
      expect(directoryInventory(notesRoot)).toEqual([]);
    },
  );

  scenario(
    "language.detect-from-content",
    "detects the language from typed content alone",
    async () => {
      await app.newSlate();
      await editor.replaceText(RUST_SOURCE);

      // Detection is content-based and debounced, and runs in Rust.
      await statusBar.waitForDetectedLanguage("rust");
      await statusBar.waitForLanguageMode("rust");
    },
  );

  scenario(
    "language.save-extension",
    "names the autosaved slate with the canonical extension for its language",
    async () => {
      await app.newSlate();
      await editor.replaceText(SQL_SOURCE);
      await statusBar.waitForDetectedLanguage("sql");

      const saved = await waitForAutosavedFile(SQL_SOURCE);
      // Naming is per-language and owned by the Rust naming registry.
      expect(path.extname(saved)).toBe(".sql");
      expect(path.dirname(saved)).toBe(notesRoot);
    },
  );

  scenario(
    "file.slate.reopen-from-sidebar",
    "reopens an autosaved slate from the sidebar with its content and language",
    async () => {
      await app.newSlate();
      const source = `${RUST_SOURCE}// reopen\n`;
      await editor.replaceText(source);
      await statusBar.waitForDetectedLanguage("rust");
      const saved = await waitForAutosavedFile(source);

      // Switch away, then come back through the library rather than by path.
      await app.newSlate();
      await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });

      await sidebar.ensureOpen();
      await sidebar.openCard(saved);

      await editor.waitUntilReady({ documentPath: saved });
      await editor.waitForText(
        (text) => text.includes("pub struct Config"),
        "Reopening from the sidebar did not restore the slate's content.",
      );
      await statusBar.waitForLanguageMode("rust");
      expect(fs.readFileSync(saved, "utf8")).toBe(source);
    },
  );

  scenario(
    "sidebar.filter-tabs",
    "lists an autosaved slate under All and Slates but never under Local",
    async () => {
      await app.newSlate();
      // Distinct from the other scenarios' content: the probe below looks for
      // exactly one matching file, and identical text across scenarios in this
      // file would match several.
      const source = `${RUST_SOURCE}// filter tabs\n`;
      await editor.replaceText(source);
      const saved = await waitForAutosavedFile(source);

      await sidebar.ensureOpen();
      await sidebar.setFilterTab("unified");
      await sidebar.waitForCard(saved);
      await sidebar.setFilterTab("slates");
      await sidebar.waitForCard(saved);
      // A managed slate is not a local file.
      await sidebar.setFilterTab("local");
      await sidebar.waitForCard(saved, false);
      await sidebar.setFilterTab("unified");
    },
  );
});

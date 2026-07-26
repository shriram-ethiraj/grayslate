import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  invokeInApp,
  openAuthorizedPath,
  pressMod,
  readEditorText,
  reopenWithEncoding,
  replaceEditorText,
  saveWithEncoding,
  type DocumentDescriptor,
  waitForDirtyState,
  waitForEditorReady,
  waitForEditorText,
  waitForEncoding,
  waitForSaveActionEnabled,
} from "../helpers/app.js";
import { homeDirectory, notesRoot } from "../helpers/sandbox.js";

const externalRoot = path.join(homeDirectory, "external-encoding");

async function grantAndOpen(filePath: string): Promise<DocumentDescriptor> {
  const descriptor = await invokeInApp<DocumentDescriptor | null>("e2e_open_path", {
    path: filePath,
  });
  if (!descriptor) throw new Error("Encoding fixture did not receive a document grant.");
  return descriptor;
}

async function waitForRawFile(
  filePath: string,
  expected: Buffer,
  timeoutMs = 15_000,
): Promise<void> {
  await browser.waitUntil(
    () => {
      try {
        return fs.readFileSync(filePath).equals(expected);
      } catch {
        return false;
      }
    },
    {
      timeout: timeoutMs,
      interval: 200,
      timeoutMsg: `Raw file bytes never matched for ${filePath}.`,
    },
  );
}

describe("Act 13 — character encoding", () => {
  before(() => {
    fs.mkdirSync(externalRoot, { recursive: true });
  });

  it("detects UTF-8 BOM without exposing the BOM as editor text", async () => {
    const filePath = path.join(externalRoot, "utf8-bom.txt");
    const original = Buffer.concat([
      Buffer.from([0xEF, 0xBB, 0xBF]),
      Buffer.from("alpha\nbeta\n", "utf8"),
    ]);
    fs.writeFileSync(filePath, original);

    const descriptor = await grantAndOpen(filePath);
    await waitForEditorReady({
      documentPath: descriptor.displayPath,
      documentLength: "alpha\nbeta\n".length,
    });
    await waitForEncoding("utf-8-bom");
    expect((await readEditorText()).trimEnd()).toBe("alpha\nbeta");
    expect(fs.readFileSync(filePath)).toEqual(original);
  });

  it("detects UTF-16 LE and converts it explicitly to UTF-8", async () => {
    const filePath = path.join(externalRoot, "utf16-le.txt");
    const text = "alpha\nβeta\n";
    fs.writeFileSync(filePath, Buffer.from(`\uFEFF${text}`, "utf16le"));

    const descriptor = await grantAndOpen(filePath);
    await waitForEditorReady({
      documentPath: descriptor.displayPath,
      documentLength: text.length,
    });
    await waitForEncoding("utf-16le");

    await saveWithEncoding("utf-8");
    expect(fs.readFileSync(filePath)).toEqual(Buffer.from(text, "utf8"));
  });

  it("asks before interpreting invalid UTF-8 as Windows-1252", async () => {
    const filePath = path.join(externalRoot, "windows-1252.txt");
    const original = Buffer.from([0x63, 0x61, 0x66, 0xE9]);
    fs.writeFileSync(filePath, original);

    const descriptor = await grantAndOpen(filePath);
    const confirmation = await $("[data-testid='encoding-confirmation-dialog']");
    await confirmation.waitForDisplayed();
    await clickTestId("encoding-confirmation-accept");
    await waitForEditorReady({
      documentPath: descriptor.displayPath,
      documentLength: 4,
    });
    await waitForEncoding("windows-1252");
    expect(await readEditorText()).toBe("café");
    expect(fs.readFileSync(filePath)).toEqual(original);
  });

  it("refuses a lossy Windows-1252 save and keeps the original bytes", async () => {
    const filePath = path.join(externalRoot, "windows-1252-loss.txt");
    const original = Buffer.from([0x63, 0x61, 0x66, 0xE9]);
    fs.writeFileSync(filePath, original);

    const descriptor = await grantAndOpen(filePath);
    await $("[data-testid='encoding-confirmation-dialog']").waitForDisplayed();
    await clickTestId("encoding-confirmation-accept");
    await waitForEditorReady({
      documentPath: descriptor.displayPath,
      documentLength: 4,
    });
    await replaceEditorText("café β");
    await waitForDirtyState(true);
    await pressMod("s");

    await browser.waitUntil(
      async () => {
        const toasts = await browser.$$("[data-sonner-toast]");
        const messages = await toasts.map((toast) => toast.getText());
        return messages.some((message) => message.includes("cannot be represented"));
      },
      {
        timeout: 10_000,
        interval: 200,
        timeoutMsg: "The strict encoding error was not shown.",
      },
    );
    expect(fs.readFileSync(filePath)).toEqual(original);
    await waitForDirtyState(true);

    // Recovery is explicit and lossless: choosing UTF-8 writes the same live
    // text and leaves the test session clean for normal application teardown.
    await saveWithEncoding("utf-8");
    expect(fs.readFileSync(filePath)).toEqual(Buffer.from("café β", "utf8"));
    await waitForDirtyState(false);
  });

  it("saves a dirty local file before reopen, then tracks and saves post-reload edits", async () => {
    const filePath = path.join(externalRoot, "local-reopen-lifecycle.txt");
    const beforeReopen = "alpha\nsaved before reopen\n";
    const afterReopen = "alpha\nsaved before reopen\ncafé\n";
    fs.writeFileSync(filePath, "alpha\n", "utf8");

    const descriptor = await grantAndOpen(filePath);
    await waitForEditorReady({
      documentPath: descriptor.displayPath,
      documentLength: "alpha\n".length,
    });
    await waitForEncoding("utf-8");
    await waitForDirtyState(false);
    await waitForSaveActionEnabled(false);

    await replaceEditorText(beforeReopen);
    await waitForDirtyState(true);
    await waitForSaveActionEnabled(true);

    await clickTestId("status-encoding");
    const encodingDialog = await $("[data-testid='encoding-picker-dialog']");
    await encodingDialog.waitForDisplayed();
    await clickTestId("encoding-select-trigger");
    await clickTestId("encoding-item-windows-1252");

    const reopenAction = await $("[data-testid='encoding-reopen']");
    expect(await reopenAction.isEnabled()).toBe(true);
    expect(await encodingDialog.getText()).not.toContain(
      "Save or discard your changes before reopening.",
    );

    await reopenAction.click();
    await encodingDialog.waitForDisplayed({ reverse: true });
    const unsavedDialog = await $("[data-testid='unsaved-changes-dialog']");
    await unsavedDialog.waitForDisplayed();
    expect(await readEditorText()).toBe(beforeReopen);

    await clickTestId("unsaved-save");
    await unsavedDialog.waitForDisplayed({ reverse: true });
    await waitForEncoding("windows-1252");
    await waitForEditorText((text) => text === beforeReopen);
    await waitForRawFile(filePath, Buffer.from(beforeReopen, "utf8"));
    await waitForDirtyState(false);
    await waitForSaveActionEnabled(false);

    await replaceEditorText(afterReopen);
    await waitForDirtyState(true);
    await waitForSaveActionEnabled(true);
    await pressMod("s");

    const expectedWindows1252 = Buffer.concat([
      Buffer.from("alpha\nsaved before reopen\ncaf", "ascii"),
      Buffer.from([0xE9]),
      Buffer.from("\n", "ascii"),
    ]);
    await waitForRawFile(filePath, expectedWindows1252);
    expect(fs.readFileSync(filePath)).toEqual(expectedWindows1252);
    expect(fs.readFileSync(filePath).includes(Buffer.from([0xC3, 0xA9]))).toBe(false);
    await waitForDirtyState(false);
    await waitForSaveActionEnabled(false);
  });

  it("silently saves a managed slate before reopen and keeps autosaving in the new encoding", async () => {
    const filePath = path.join(notesRoot, "slate-reopen-lifecycle.txt");
    const beforeReopen = "slate\nsaved before reopen\n";
    const afterReopen = "slate\nsaved before reopen\ncafé\n";
    fs.writeFileSync(filePath, "slate\n", "utf8");

    const descriptor = await openAuthorizedPath(filePath);
    expect(descriptor.source).toBe("slates");
    await waitForEncoding("utf-8");
    await replaceEditorText(beforeReopen);

    await reopenWithEncoding("windows-1252");
    expect(await $("[data-testid='unsaved-changes-dialog']").isExisting()).toBe(false);
    await waitForEncoding("windows-1252");
    await waitForEditorText((text) => text === beforeReopen);
    await waitForRawFile(filePath, Buffer.from(beforeReopen, "utf8"));

    await replaceEditorText(afterReopen);
    const expectedWindows1252 = Buffer.concat([
      Buffer.from("slate\nsaved before reopen\ncaf", "ascii"),
      Buffer.from([0xE9]),
      Buffer.from("\n", "ascii"),
    ]);
    await waitForRawFile(filePath, expectedWindows1252, 20_000);
    expect(fs.readFileSync(filePath)).toEqual(expectedWindows1252);
  });
});

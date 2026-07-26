import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  invokeInApp,
  pressMod,
  readEditorText,
  replaceEditorText,
  saveWithEncoding,
  type DocumentDescriptor,
  waitForDirtyState,
  waitForEditorReady,
  waitForEncoding,
} from "../helpers/app.js";
import { homeDirectory } from "../helpers/sandbox.js";

const externalRoot = path.join(homeDirectory, "external-encoding");

async function grantAndOpen(filePath: string): Promise<DocumentDescriptor> {
  const descriptor = await invokeInApp<DocumentDescriptor | null>("e2e_open_path", {
    path: filePath,
  });
  if (!descriptor) throw new Error("Encoding fixture did not receive a document grant.");
  return descriptor;
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
});

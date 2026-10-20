import fs from "node:fs";
import { browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import { activatePaths, dropPaths, openText, provisionBytes, provisionText, queueOpenDialogResults } from "../fixtures/factories.js";
import { readEditorReadiness, readPendingWork, readNativeWindowStates } from "../driver/probe.js";
import { waitFor, waitForAppStable } from "../driver/wait.js";
import { armOperationGate, waitForOperationGate, releaseOperationGate } from "../driver/operationGate.js";
import { TIMEOUTS } from "../config/timeouts.js";
import { applicationRssKiB, recordOpeningBenchmark } from "../helpers/openingBenchmark.js";
import * as titleBar from "../pages/titleBar.js";
import * as editor from "../pages/editor.js";
import * as dialogs from "../pages/dialogs.js";

async function closeOtherWindows(original: string): Promise<void> {
  for (const handle of await browser.getWindowHandles()) {
    if (handle === original) continue;
    await browser.switchToWindow(handle);
    await waitForAppStable();
    await titleBar.closeWindow();
  }
  await browser.switchToWindow(original);
  await waitFor(async () => (await browser.getWindowHandles()).length === 1, {
    timeoutMs: TIMEOUTS.ui, message: "Batch windows did not finish closing.",
  });
}

describe("Opening batches", () => {
  scenario("file.batch.reuses-pending-owner", "shares a pending owner across independent window queues", async () => {
    const original = await browser.getWindowHandle();
    await titleBar.fileMenu("new-window");
    await waitFor(async () => (await browser.getWindowHandles()).length === 2, {
      timeoutMs: TIMEOUTS.ui, message: "The second receiving window did not appear.",
    });
    const second = (await browser.getWindowHandles()).find((handle) => handle !== original);
    if (!second) throw new Error("The second receiving window disappeared.");
    await browser.switchToWindow(second);
    await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
    await browser.switchToWindow(original);
    const target = provisionText("shared-pending-owner.txt", "one pending owner\n");
    await armOperationGate("file-read", target);
    try {
      await queueOpenDialogResults([target]);
      await titleBar.fileMenu("open-file-new-window");
      await waitForOperationGate("file-read");
      await browser.switchToWindow(second);
      const rest = Array.from({ length: 5 }, (_, index) => provisionText(`beside-pending-${index}.txt`, `beside pending ${index}\n`));
      await queueOpenDialogResults([target, ...rest]);
      await titleBar.fileMenu("open-file-new-window");
      await waitFor(async () => {
        const pending = await readPendingWork();
        return pending?.commands.includes("wait_document_open") === true
          && !pending.commands.some((command) => command.startsWith("create_editor_window"))
          && (await browser.getWindowHandles()).length === 8;
      }, { timeoutMs: TIMEOUTS.heavy, message: "A reused pending owner blocked other slots or received a duplicate window." });
    } finally {
      await releaseOperationGate("file-read");
    }
    await waitForAppStable();
    await browser.switchToWindow(original);
    await waitForAppStable();
    expect(await browser.getWindowHandles()).toHaveLength(8);
    await closeOtherWindows(original);
  });
  scenario("file.batch.pending-encoding", "keeps an encoding confirmation in its slot while other files finish", async () => {
    const original = await browser.getWindowHandle();
    const paths = Array.from({ length: 6 }, (_, index) => provisionText(`encoding-slot-${index}.txt`, `encoding slot ${index}\n`));
    paths[0] = provisionBytes("encoding-slot-legacy.txt", Buffer.from([0x63, 0x61, 0x66, 0xe9]));
    await queueOpenDialogResults(paths);
    await armOperationGate("file-read", paths[1]);
    try {
      await titleBar.fileMenu("open-file-new-window");
      await waitForOperationGate("file-read");
      await waitFor(async () => {
        const pending = await readPendingWork();
        return pending?.commands.includes("wait_document_open (2)") === true
          && !pending.commands.some((command) => command.startsWith("create_editor_window"))
          && (await browser.getWindowHandles()).length === 7;
      }, { timeoutMs: TIMEOUTS.heavy, message: "Other files did not finish around the pending encoding and read slots." });
      let cancelled = false;
      for (const handle of await browser.getWindowHandles()) {
        if (handle === original) continue;
        await browser.switchToWindow(handle);
        if (await dialogs.encodingConfirmation.isOpen()) {
          await dialogs.encodingConfirmation.cancel();
          cancelled = true;
          break;
        }
      }
      expect(cancelled).toBe(true);
      await browser.switchToWindow(original);
      await waitFor(async () => (await readPendingWork())?.commands.includes("wait_document_open") === true, {
        timeoutMs: TIMEOUTS.ui, message: "Encoding cancellation did not release its loading slot.",
      });
    } finally {
      await releaseOperationGate("file-read");
    }
    await waitForAppStable();
    await waitFor(async () => (await readNativeWindowStates()).some((state) => state.is_focused && state.title.includes("encoding-slot-5.txt")), {
      timeoutMs: TIMEOUTS.ui, message: "The cancelled encoding file overrode final-selection focus.",
    });
    expect(fs.readFileSync(paths[0])).toEqual(Buffer.from([0x63, 0x61, 0x66, 0xe9]));
    await closeOtherWindows(original);
  });
  scenario("file.batch.guard-before-loads", "resolves Save, Discard, and Cancel before starting any batch load", async () => {
    const original = await browser.getWindowHandle();
    for (const choice of ["cancel", "discard", "save"] as const) {
      const source = await openText(`guard-${choice}.txt`, "saved source\n");
      await editor.replaceText("unsaved source\n");
      const paths = [0, 1].map((index) => provisionText(`guard-${choice}-${index}.txt`, `guard target ${index}\n`));
      await queueOpenDialogResults(paths);
      await armOperationGate("file-read");
      try {
        await titleBar.fileMenu("open-file");
        await dialogs.unsavedChanges.waitForOpen();
        const pending = await readPendingWork();
        expect(pending?.commands.some((command) => /^(read_file_content|create_editor_window|wait_document_open)/.test(command))).toBe(false);
        expect(await browser.getWindowHandles()).toHaveLength(1);
      } finally {
        await releaseOperationGate("file-read");
      }
      await dialogs.unsavedChanges[choice]();
      await dialogs.unsavedChanges.waitForClosed();
      await waitForAppStable();
      if (choice === "cancel") {
        expect(await browser.getWindowHandles()).toHaveLength(1);
        await editor.waitForExactText("unsaved source\n");
        expect(fs.readFileSync(source, "utf8")).toBe("saved source\n");
        await titleBar.fileMenu("save");
        await titleBar.waitForDirty(false);
      } else {
        await editor.waitUntilReady({ documentPath: paths[1] });
        expect(fs.readFileSync(source, "utf8")).toBe(choice === "save" ? "unsaved source\n" : "saved source\n");
        await closeOtherWindows(original);
      }
    }
  });
  scenario("file.batch.four-load-limit", "shares four slots with the receiving load and refills across multiple waves", async () => {
    const original = await browser.getWindowHandle();
    for (const mode of ["new-windows", "picker", "drop", "activation"] as const) {
      const receiving = mode === "picker" || mode === "drop";
      const paths = Array.from({ length: 9 }, (_, index) => provisionText(`bounded-${mode}-${index}.txt`, `bounded ${index}\n`));
      if (mode === "new-windows" || mode === "picker") await queueOpenDialogResults(paths);
      await armOperationGate("file-read");
      try {
        if (mode === "drop") await dropPaths(paths);
        else if (mode === "activation") await activatePaths(paths);
        else await titleBar.fileMenu(receiving ? "open-file" : "open-file-new-window");
        await waitForOperationGate("file-read");
        await waitFor(async () => {
          const pending = await readPendingWork();
          return pending?.commands.includes(`wait_document_open (${receiving ? 3 : 4})`) === true
            && !pending.commands.some((command) => command.startsWith("create_editor_window"))
            && (!receiving || pending.commands.includes("read_file_content"));
        }, { timeoutMs: TIMEOUTS.editor, message: "Four file loads never overlapped at the read gate." });
        expect(await browser.getWindowHandles()).toHaveLength(receiving ? 4 : 5);
        // Observe every secondary read actually held in Rust, rather than
        // inferring overlap merely from native window creation.
        for (const handle of await browser.getWindowHandles()) {
          if (handle === original) continue;
          await browser.switchToWindow(handle);
          await waitFor(async () => (await readPendingWork())?.commands.includes("read_file_content") === true, {
            timeoutMs: TIMEOUTS.editor, message: "A secondary window never reached its held file read.",
          });
        }
        await browser.switchToWindow(original);
      } finally {
        await browser.switchToWindow(original);
        await releaseOperationGate("file-read");
      }
      await waitForAppStable({ timeoutMs: TIMEOUTS.heavy });
      expect(await browser.getWindowHandles()).toHaveLength(receiving ? 9 : 10);
      if (receiving) await editor.waitUntilReady({ documentPath: paths.at(-1) });
      await waitFor(async () => (await readNativeWindowStates()).some((state) => state.is_focused && state.title.includes(`bounded-${mode}-8.txt`)), {
        timeoutMs: TIMEOUTS.ui, message: "The final selection did not receive native focus.",
      });
      await closeOtherWindows(original);
    }
  });

  scenario("file.batch.blocked-read", "opens later waves while the first read stays blocked and focuses the final selection", async () => {
    const original = await browser.getWindowHandle();
    for (const receiving of [false, true]) {
      const paths = Array.from({ length: 9 }, (_, index) => provisionText(`overlap-${receiving}-${index}.txt`, `overlap ${index}\n`));
      paths[1] = provisionBytes(`overlap-failed-${receiving}.bin`, Buffer.from([0xff, 0xfe, 0, 0, 65, 0, 0, 0]));
      await queueOpenDialogResults(paths);
      await armOperationGate("file-read", paths[0]);
      try {
        await titleBar.fileMenu(receiving ? "open-file" : "open-file-new-window");
        await waitForOperationGate("file-read");
        await waitFor(async () => {
          const pending = await readPendingWork();
          return pending?.commands.includes("wait_document_open") === true
            && !pending.commands.some((command) => command.startsWith("create_editor_window"))
            && (await browser.getWindowHandles()).length === (receiving ? 9 : 10);
        }, { timeoutMs: TIMEOUTS.heavy, message: "The blocked first file prevented later waves from opening." });
        // The final selection is ready before the first read is released,
        // including when that final selection belongs to the receiving window.
        if (!receiving) {
          let foundFinal = false;
          for (const handle of await browser.getWindowHandles()) {
            if (handle === original) continue;
            await browser.switchToWindow(handle);
            if (await titleBar.documentPath() === paths.at(-1)) {
              await editor.waitUntilReady({ documentPath: paths.at(-1) });
              await editor.waitForExactText("overlap 8\n");
              foundFinal = true;
            }
          }
          expect(foundFinal).toBe(true);
          await browser.switchToWindow(original);
        } else {
          // This window owns the still-running batch, so an application-idle
          // helper would wait for the read we deliberately have not released.
          await waitFor(async () => {
            const snapshot = await readEditorReadiness();
            return snapshot.ready && snapshot.documentPath === paths.at(-1);
          }, { timeoutMs: TIMEOUTS.editor, message: "The receiving document did not become ready ahead of the blocked file." });
          await editor.waitForExactText("overlap 8\n");
        }
      } finally {
        await browser.switchToWindow(original);
        await releaseOperationGate("file-read");
      }
      await waitForAppStable();
      await waitFor(async () => (await readNativeWindowStates()).some((state) => state.is_focused && state.title.includes(`overlap-${receiving}-8.txt`)), {
        timeoutMs: TIMEOUTS.ui, message: "Completion order replaced selection order for final focus.",
      });
      await closeOtherWindows(original);
    }
  });
  scenario("file.batch.benchmark", "opens eight and sixteen files while the receiving window stays responsive", async () => {
    const original = await browser.getWindowHandle();
    for (const count of [8, 16]) {
      const paths = Array.from({ length: count }, (_, index) => provisionText(`benchmark-${count}-${index}.txt`, `batch ${count} document ${index}\n`.repeat(1_000)));
      await queueOpenDialogResults(paths);
      const beforeRssKiB = applicationRssKiB();
      let peakRssKiB = beforeRssKiB;
      const latenciesMs: number[] = [];
      const started = performance.now();
      await titleBar.fileMenu("open-file-new-window");
      await waitFor(async () => {
        const probeStarted = performance.now();
        const pending = await readPendingWork();
        latenciesMs.push(performance.now() - probeStarted);
        const rss = applicationRssKiB();
        if (rss !== null) peakRssKiB = Math.max(peakRssKiB ?? 0, rss);
        return pending?.phase === "ready" && pending.inFlight === 0;
      }, { timeoutMs: TIMEOUTS.heavy, message: `The ${count}-file benchmark did not settle.` });
      const elapsedMs = performance.now() - started;
      expect(await browser.getWindowHandles()).toHaveLength(count + 1);
      // The initiating window must retain its document and remain queryable.
      await editor.waitUntilReady();
      recordOpeningBenchmark({ mode: process.env.OPENING_BENCHMARK_LABEL ?? "unspecified", count, elapsedMs, beforeRssKiB, peakRssKiB, maxProbeMs: Math.max(...latenciesMs), samples: latenciesMs.length });
      await closeOtherWindows(original);
    }
  });
});

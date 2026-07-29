import { TIMEOUTS } from "../config/timeouts.js";
import { browser } from "@wdio/globals";
import { invokeInApp } from "../driver/invoke.js";
import { setValueTestId } from "../driver/interact.js";
import { DELETE, pressMod } from "../driver/keys.js";
import { waitFor } from "../driver/wait.js";
import {
  byTestId,
  clickTestId,
  isDisplayedTestId,
  pressEscape,
  textOf,
  waitForTestId,
} from "./common.js";

/** The app's modal dialogs. */

// ── Unsaved changes guard ──────────────────────────────────────────────────

export const unsavedChanges = {
  async waitForOpen(): Promise<void> {
    await waitForTestId("unsaved-changes-dialog", { timeoutMs: TIMEOUTS.ui });
  },
  async waitForClosed(): Promise<void> {
    await waitForTestId("unsaved-changes-dialog", { reverse: true });
  },
  async isOpen(): Promise<boolean> {
    return isDisplayedTestId("unsaved-changes-dialog");
  },
  async text(): Promise<string> {
    return textOf("unsaved-changes-dialog");
  },
  async save(): Promise<void> {
    await clickTestId("unsaved-save");
  },
  async discard(): Promise<void> {
    await clickTestId("unsaved-discard");
  },
  async cancel(): Promise<void> {
    await clickTestId("unsaved-cancel");
  },
};

// ── Delete confirmation ────────────────────────────────────────────────────

export const deleteFile = {
  async waitForOpen(): Promise<void> {
    await waitForTestId("delete-file-dialog");
  },
  async isOpen(): Promise<boolean> {
    return isDisplayedTestId("delete-file-dialog");
  },
  async confirm(): Promise<void> {
    await clickTestId("delete-confirm");
    await this.waitForClosed();
  },
  async cancel(): Promise<void> {
    await clickTestId("delete-cancel");
    await this.waitForClosed();
  },
  async waitForClosed(): Promise<void> {
    await waitForTestId("delete-file-dialog", { reverse: true });
  },
};

// ── Rename ─────────────────────────────────────────────────────────────────

export const renameFile = {
  async waitForOpen(): Promise<void> {
    await waitForTestId("rename-file-dialog");
  },
  async setName(name: string): Promise<void> {
    const input = await byTestId("rename-input");
    await input.waitForDisplayed({
      timeout: TIMEOUTS.ui,
      timeoutMsg: "The rename input never appeared.",
    });
    if (name === "") {
      // `clearValue()` empties the DOM value without driving Svelte's binding,
      // so the component still held the old name and the "empty" submit
      // actually succeeded. Select-all + Delete is what a user does, and it
      // produces the input events the binding listens for.
      await input.click();
      await pressMod("a");
      await browser.keys(DELETE);
      await waitFor(async () => (await input.getValue()) === "", {
        message: "The rename input was not cleared.",
        timeoutMs: TIMEOUTS.ui,
      });
      return;
    }
    await setValueTestId("rename-input", name);
  },
  async value(): Promise<string> {
    return (await byTestId("rename-input")).getValue();
  },
  /** Ask the backend to suggest a name from the file's content. */
  async generateName(): Promise<void> {
    await clickTestId("rename-generate");
  },
  /** Submit and wait for the dialog to close. Invalid input keeps it open. */
  async submit(): Promise<void> {
    await clickTestId("rename-submit");
  },
  async cancel(): Promise<void> {
    await clickTestId("rename-cancel");
    await this.waitForClosed();
  },
  async waitForClosed(): Promise<void> {
    await waitForTestId("rename-file-dialog", { reverse: true });
  },
  async isOpen(): Promise<boolean> {
    return isDisplayedTestId("rename-file-dialog");
  },
  async text(): Promise<string> {
    return textOf("rename-file-dialog");
  },
};

// ── Encoding confirmation (ambiguous decode) ───────────────────────────────

export const encodingConfirmation = {
  async waitForOpen(): Promise<void> {
    await waitForTestId("encoding-confirmation-dialog", { timeoutMs: TIMEOUTS.editor });
  },
  async isOpen(): Promise<boolean> {
    return isDisplayedTestId("encoding-confirmation-dialog");
  },
  async text(): Promise<string> {
    return textOf("encoding-confirmation-dialog");
  },
  async accept(): Promise<void> {
    await clickTestId("encoding-confirmation-accept");
  },
};

// ── Go to line ─────────────────────────────────────────────────────────────

export const goToLine = {
  async open(): Promise<void> {
    await clickTestId("status-goto-line");
    await waitForTestId("go-to-line-dialog");
  },
  async enter(line: number): Promise<void> {
    const input = await byTestId("go-to-line-input");
    await input.waitForDisplayed({
      timeout: TIMEOUTS.ui,
      timeoutMsg: "The go-to-line input never appeared.",
    });
    await input.setValue(String(line));
  },
  async isInvalid(): Promise<boolean> {
    return (await (await byTestId("go-to-line-input")).getAttribute("aria-invalid")) === "true";
  },
};

// ── About ──────────────────────────────────────────────────────────────────

export const about = {
  async waitForOpen(): Promise<void> {
    await waitForTestId("about-dialog");
  },
  async close(): Promise<void> {
    await pressEscape();
    await this.waitForClosed();
  },
  async waitForClosed(): Promise<void> {
    await waitForTestId("about-dialog", { reverse: true });
  },
  async text(): Promise<string> {
    return textOf("about-dialog");
  },
  async updateStatus(): Promise<string | null> {
    return (await byTestId("about-update-status")).getAttribute("data-update-status");
  },
  async updatePolicy(): Promise<string | null> {
    return (await byTestId("about-update-status")).getAttribute("data-update-policy");
  },
  async hasInstallAction(): Promise<boolean> {
    return isDisplayedTestId("about-install-update");
  },
  async checkForUpdates(): Promise<void> {
    await clickTestId("about-check-updates");
  },
  async appInfo(): Promise<{
    appName: string;
    appVersion: string;
    updatePolicy: string;
  }> {
    return invokeInApp("get_app_info");
  },
};

// ── Keyboard shortcuts ─────────────────────────────────────────────────────

export const keyboardShortcuts = {
  async waitForOpen(): Promise<void> {
    await waitForTestId("keyboard-shortcuts-dialog");
  },
  async search(query: string): Promise<void> {
    const input = await byTestId("keyboard-shortcuts-search");
    await input.waitForDisplayed({
      timeout: TIMEOUTS.ui,
      timeoutMsg: "The shortcuts search input never appeared.",
    });
    await input.setValue(query);
  },
  async listText(): Promise<string> {
    return textOf("keyboard-shortcuts-list");
  },
};

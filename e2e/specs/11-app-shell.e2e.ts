import { browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import { invokeInApp } from "../driver/invoke.js";
import { waitForProcessSignal } from "../driver/processSignal.js";
import { readComputedStyleBySelector } from "../driver/probe.js";
import { waitFor } from "../driver/wait.js";
import { openText } from "../fixtures/factories.js";
import { minimizeObservationPath } from "../helpers/sandbox.js";
import { existsTestId } from "../pages/common.js";
import * as app from "../pages/app.js";
import * as dialogs from "../pages/dialogs.js";
import * as editor from "../pages/editor.js";
import * as settings from "../pages/settings.js";
import * as statusBar from "../pages/statusBar.js";
import * as titleBar from "../pages/titleBar.js";
import * as transformations from "../pages/transformations.js";

/** Native window state, as the OS sees it rather than as the app believes. */
async function nativeWindowState(): Promise<{ maximized: boolean; minimized: boolean }> {
  // ipc-oracle: window state is owned by the compositor and has no DOM
  // representation, so there is nothing for WebDriver to read. The action under
  // test is still a real click on the title-bar control.
  const maximized = await invokeInApp<boolean>("plugin:window|is_maximized", {
    label: "main",
  });
  const minimized = await invokeInApp<boolean>("plugin:window|is_minimized", {
    label: "main",
  });
  return { maximized, minimized };
}

describe("App shell and lifecycle", () => {
  scenario("shell.about", "opens About and reports its update state", async () => {
    await titleBar.helpMenu("about");
    await dialogs.about.waitForOpen();

    const text = await dialogs.about.text();
    const appInfo = await dialogs.about.appInfo();
    expect(text).toContain("About");
    expect(text).toContain(appInfo.appName);
    expect(text).toContain(`v${appInfo.appVersion}`);

    // The updater is a state machine; the dialog must always express one of its
    // states rather than rendering nothing.
    const status = await dialogs.about.updateStatus();
    expect(status).toBeTruthy();
    expect([
      "checking",
      "available",
      "installing",
      "installed",
      "up-to-date",
      "system-managed",
      "disabled",
      "error",
      "idle",
    ]).toContain(status);

    await dialogs.about.close();
  });

  scenario(
    "shell.updates.check",
    "exposes an update action consistent with the build's update policy",
    async () => {
      await titleBar.helpMenu("about");
      await dialogs.about.waitForOpen();

      const policy = await dialogs.about.updatePolicy();
      expect(policy).toBeTruthy();

      // Only a self-updating build offers to check; a system-managed or
      // disabled build must not pretend it can.
      const canCheck = await existsTestId("about-check-updates");
      expect(canCheck).toBe(policy === "self-update");

      if (canCheck) {
        await dialogs.about.checkForUpdates();
        await waitFor(
          async () => (await dialogs.about.updateStatus()) !== "checking",
          { message: "The update check never reached a terminal state." },
        );
      }

      await dialogs.about.close();
    },
  );

  scenario(
    "shell.window.maximize-restore",
    "maximizes and restores the native window",
    async () => {
      const wasMaximized = (await nativeWindowState()).maximized;

      await titleBar.toggleMaximizeWindow();
      await waitFor(async () => (await nativeWindowState()).maximized !== wasMaximized, {
        message:
          "The native window state did not change after clicking maximize. " +
          "A window manager must be running (see e2e/README.md).",
      });
      // The control must also reflect the new state back to the user.
      await titleBar.waitForMaximized(!wasMaximized);

      await titleBar.toggleMaximizeWindow();
      await waitFor(async () => (await nativeWindowState()).maximized === wasMaximized, {
        message: "The native window did not return to its original state.",
      });
      await titleBar.waitForMaximized(wasMaximized);
    },
  );

  scenario("shell.window.minimize", "minimizes the native window", async () => {
    await invokeInApp<void>("e2e_arm_minimize_probe");
    const minimizeRequest = titleBar.minimizeWindow();

    // A minimized WebKitGTK webview is suspended, so the observation cannot be
    // queried through in-page IPC. The E2E-only backend writes this marker only
    // after it sees the native minimized state; Node can observe it without
    // asking the suspended page to execute JavaScript.
    await waitForProcessSignal(
      minimizeObservationPath,
      "The native observer never saw the window enter its minimized state.",
    );

    // The backend observer restores the window after recording the native
    // transition. The production action under test remains the real title-bar
    // click, proven by the marker above.
    await minimizeRequest;
    // WebDriver's Set Window Rect command normalizes the restored geometry.
    await browser.setWindowRect(null, null, 1440, 900);
    await editor.waitUntilReady();

    await waitFor(async () => {
      const observation = await invokeInApp<{
        armed: boolean;
        observed: boolean;
        restored: boolean;
      }>("e2e_minimize_observation");
      return observation.observed && observation.restored && !observation.armed;
    }, {
      message: "The minimize probe did not settle after the window was restored.",
    });
    expect(await (await editor.content()).isExisting()).toBe(true);
  });

  /*
   * Theme and settings defaults are *functional* behavior, so they live here
   * rather than in `specs/visual/`. They were originally written beside the
   * typography measurements because both happen to touch appearance, but the
   * visual suite is advisory — a functional claim made from there would stop
   * gating the moment that job is made non-blocking, while still counting
   * towards coverage. The audit now rejects a cross-tier claim outright.
   */
  scenario(
    "shell.theme.toggle-and-persist",
    "toggles the theme, repaints, and keeps the choice across an editor remount",
    async () => {
      const wasDark = await app.isDarkTheme();
      const backgroundBefore = await readComputedStyleBySelector("body", ["background-color"]);

      await app.toggleTheme();
      await app.waitForTheme(!wasDark);

      // The class flipping is not enough on its own: it would still pass if the
      // stylesheet never resolved. Require the page to have actually repainted.
      const backgroundAfter = await readComputedStyleBySelector("body", ["background-color"]);
      expect(backgroundAfter?.["background-color"]).not.toBe(
        backgroundBefore?.["background-color"],
      );
      expect(await app.storedPreference("theme")).toBe(wasDark ? "light" : "dark");

      // A new slate tears down and remounts the editor; the choice must survive.
      await app.newSlate();
      expect(await app.isDarkTheme()).toBe(!wasDark);

      await app.restoreTheme(wasDark);
    },
  );

  scenario(
    "shell.settings.indent-default",
    "applies the default indentation from Settings to a new slate",
    async () => {
      await settings.open();
      await settings.setIndentMode("spaces");
      await settings.setIndentSize(4);
      await settings.close();

      await app.newSlate();
      await statusBar.waitForIndentLabel("Spaces: 4");

      // Restore the shipped default. The original version of this scenario left
      // the preference changed, and only the per-spec-file sandbox wipe stopped
      // it leaking into the next file.
      await settings.open();
      await settings.setIndentSize(2);
      await settings.close();
    },
  );

  scenario(
    "shell.window.close-guard",
    "intercepts a native close while a local document has unsaved changes",
    async () => {
      await openText("close-guard-shell.txt", "shell guard base\n");
      await editor.replaceText("shell guard: unsaved edit");
      await titleBar.waitForDirty(true);

      await titleBar.closeWindow();
      await dialogs.unsavedChanges.waitForOpen();

      // Cancel must keep both the window and the unsaved work.
      await dialogs.unsavedChanges.cancel();
      await dialogs.unsavedChanges.waitForClosed();
      expect(await (await editor.content()).isDisplayed()).toBe(true);
      expect(await titleBar.isDirty()).toBe(true);
    },
  );

  scenario(
    "shell.toasts.success-and-error",
    "shows readable success and error toasts for completed actions",
    async () => {
      const original = "{ not valid json }\n";
      await openText("toast-feedback.json", original);

      await transformations.run("text.uppercase");
      await transformations.waitForToast("Converted to uppercase.");
      await editor.focus();
      await editor.undo();
      await editor.waitForExactText(original);

      await transformations.run("json.validate");
      await transformations.waitForToastContaining("Invalid JSON:");
      expect(await editor.text()).toBe(original);
    },
  );
});

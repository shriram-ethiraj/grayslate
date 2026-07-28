import { browser, expect } from "@wdio/globals";
import { TIMEOUTS } from "../../config/timeouts.js";
import { expectSettledAbsent } from "../../assertions/matchers.js";
import { scenario } from "../../coverage/scenario.js";
import { withNavigationProbe, readSecurityHeaders } from "./probes.js";

describe("Webview security boundary", () => {
  scenario("security.headers", "serves restrictive security headers", async () => {
    const headers = await readSecurityHeaders();

    // A fetch failure would leave every header null, which would otherwise read
    // as "no dangerous permissions granted".
    expect(headers.fetchError).toBeUndefined();
    expect(headers.contentTypeOptions).toBe("nosniff");
    for (const directive of [
      "camera=()",
      "microphone=()",
      "geolocation=()",
      "display-capture=()",
      "usb=()",
      "serial=()",
      "hid=()",
      "payment=()",
    ]) {
      expect(headers.permissionsPolicy).toContain(directive);
    }
  });

  scenario(
    "security.navigation",
    "denies external top-level navigation and new windows",
    async () => {
      const initialUrl = await browser.getUrl();
      const initialHandles = await browser.getWindowHandles();

      // The probe element is injected and removed in a finally block. The
      // previous version left it in the DOM for the rest of the session.
      await withNavigationProbe(async (click) => {
        await click();

        // The old version slept 250 ms here. That made the assertion weaker the
        // slower the machine: a popup that was merely late, rather than
        // blocked, would pass. Instead require the invariant to hold across a
        // sampled quiet window, and only after the click has actually landed.
        await expectSettledAbsent({
          precondition: async () => {
            await browser.waitUntil(async () => (await browser.getUrl()).length > 0, {
              timeout: TIMEOUTS.ui,
              timeoutMsg: "The webview never reported a URL after the popup click.",
            });
          },
          invariant: async () => {
            const handles = await browser.getWindowHandles();
            const url = await browser.getUrl();
            return (
              handles.length === initialHandles.length &&
              handles.every((handle, index) => handle === initialHandles[index]) &&
              url === initialUrl
            );
          },
          message: "A target=_blank link must not open a window or change the URL.",
        });
      });

      // security-probe: a scripted top-level navigation is exactly the attack
      // being denied; it mutates no DOM and leaves nothing behind.
      await browser.execute(() => {
        window.location.assign("https://example.invalid/grayslate-navigation-probe");
      });

      await expectSettledAbsent({
        precondition: async () => {
          await browser.waitUntil(async () => (await browser.getUrl()).length > 0, {
            timeout: TIMEOUTS.ui,
            timeoutMsg: "The webview never reported a URL after the navigation attempt.",
          });
        },
        invariant: async () => (await browser.getUrl()) === initialUrl,
        message: "A top-level navigation to an external origin must be denied.",
      });
    },
  );
});

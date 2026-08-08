import { browser, expect } from "@wdio/globals";
import { scenario } from "../../coverage/scenario.js";
import {
  armNavigationProbe,
  readSecurityHeaders,
  waitForNavigationObservation,
  withNavigationProbe,
} from "./probes.js";

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

      await withNavigationProbe(async (click) => {
        await armNavigationProbe();
        await click();
        const observation = await waitForNavigationObservation("new-window");
        expect(observation.allowed).toBe(false);
        expect(observation.url).toBe("data:text/html,untrusted");
        expect(await browser.getWindowHandles()).toEqual(initialHandles);
        expect(await browser.getUrl()).toBe(initialUrl);
      });

      await armNavigationProbe();
      // security-probe: a scripted top-level navigation is the hostile action
      // whose Rust allow/deny decision this scenario observes.
      await browser.execute(() => {
        window.location.assign("https://example.invalid/grayslate-navigation-probe");
      });
      const observation = await waitForNavigationObservation("navigation");
      expect(observation.allowed).toBe(false);
      expect(observation.url).toBe(
        "https://example.invalid/grayslate-navigation-probe",
      );
      expect(await browser.getUrl()).toBe(initialUrl);
    },
  );
});

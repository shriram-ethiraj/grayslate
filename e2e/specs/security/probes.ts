import { $, browser } from "@wdio/globals";
import { TIMEOUTS } from "../../config/timeouts.js";

/**
 * Probes for the webview security boundary.
 *
 * These are the one sanctioned exception to "never mutate the app's DOM": the
 * threat model *is* hostile content inside the webview, so the test has to
 * introduce some. The rules are that the mutation is isolated to this module
 * and always removed in a `finally`, so no probe survives into another test —
 * the previous version leaked its `<a>` for the rest of the session.
 */

const PROBE_ID = "webview-security-popup-probe";

export interface SecurityHeaders {
  contentTypeOptions: string | null;
  permissionsPolicy: string | null;
  fetchError?: string;
}

export async function readSecurityHeaders(): Promise<SecurityHeaders> {
  // security-probe: the response headers are only observable from inside the
  // webview's own fetch; read-only, nothing is mutated.
  return browser.executeAsync((done) => {
    fetch(window.location.href, { cache: "no-store" })
      .then((response) =>
        done({
          contentTypeOptions: response.headers.get("x-content-type-options"),
          permissionsPolicy: response.headers.get("permissions-policy"),
        }),
      )
      .catch((error: unknown) =>
        done({
          contentTypeOptions: null,
          permissionsPolicy: null,
          fetchError: String(error),
        }),
      );
  }) as Promise<SecurityHeaders>;
}

/**
 * Inject a `target="_blank"` link, hand the caller a way to click it, and
 * always remove it afterwards.
 */
export async function withNavigationProbe(
  body: (click: () => Promise<void>) => Promise<void>,
): Promise<void> {
  // security-probe: the threat model is hostile content inside the webview, so
  // the test must introduce some. Removed in the finally below.
  await browser.execute((id) => {
    document.getElementById(id)?.remove();
    const link = document.createElement("a");
    link.id = id;
    link.href = "data:text/html,untrusted";
    link.target = "_blank";
    link.textContent = "popup probe";
    link.style.position = "fixed";
    link.style.inset = "0 auto auto 0";
    link.style.zIndex = "2147483647";
    document.body.append(link);
  }, PROBE_ID);

  try {
    await body(async () => {
      const probe = await $(`#${PROBE_ID}`);
      await probe.waitForClickable({
        timeout: TIMEOUTS.ui,
        timeoutMsg: "The injected navigation probe never became clickable.",
      });
      await probe.click();
    });
  } finally {
    // security-probe: mandatory cleanup so no injected node survives the test.
    await browser
      .execute((id) => document.getElementById(id)?.remove(), PROBE_ID)
      .catch(() => {
        // The session may already be gone; the sandbox reset covers the rest.
      });
  }
}

import { $, browser, expect } from "@wdio/globals";

interface SecurityHeaders {
  contentTypeOptions: string | null;
  permissionsPolicy: string | null;
  fetchError?: string;
}

async function readMainDocumentSecurityHeaders(): Promise<SecurityHeaders> {
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

describe("Webview security boundary", () => {
  it("serves restrictive security headers", async () => {
    const headers = await readMainDocumentSecurityHeaders();
    expect(headers.fetchError).toBeUndefined();
    expect(headers.contentTypeOptions).toBe("nosniff");
    expect(headers.permissionsPolicy).toContain("camera=()");
    expect(headers.permissionsPolicy).toContain("microphone=()");
    expect(headers.permissionsPolicy).toContain("geolocation=()");
    expect(headers.permissionsPolicy).toContain("display-capture=()");
    expect(headers.permissionsPolicy).toContain("usb=()");
    expect(headers.permissionsPolicy).toContain("serial=()");
    expect(headers.permissionsPolicy).toContain("hid=()");
    expect(headers.permissionsPolicy).toContain("payment=()");
  });

  it("denies external top-level navigation and new windows", async () => {
    const initialUrl = await browser.getUrl();
    const initialHandles = await browser.getWindowHandles();

    await browser.execute(() => {
      const popupLink = document.createElement("a");
      popupLink.id = "webview-security-popup-probe";
      popupLink.href = "data:text/html,untrusted";
      popupLink.target = "_blank";
      popupLink.textContent = "popup probe";
      popupLink.style.position = "fixed";
      popupLink.style.inset = "0 auto auto 0";
      popupLink.style.zIndex = "2147483647";
      document.body.append(popupLink);
    });
    await $("#webview-security-popup-probe").click();
    await browser.pause(250);

    expect(await browser.getWindowHandles()).toEqual(initialHandles);
    expect(await browser.getUrl()).toBe(initialUrl);

    await browser.execute(() => {
      window.location.assign("https://example.invalid/grayslate-navigation-probe");
    });
    await browser.pause(250);

    expect(await browser.getUrl()).toBe(initialUrl);
  });
});

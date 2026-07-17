import { browser, expect } from "@wdio/globals";

interface TauriInternals {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

interface InvokeResult<T> {
  value?: T;
  error?: string;
}

async function rawInvoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<InvokeResult<T>> {
  try {
    return await browser.executeAsync((name, payload, done) => {
      const internals = (window as unknown as { __TAURI_INTERNALS__: TauriInternals })
        .__TAURI_INTERNALS__;
      internals
        .invoke<T>(name, payload)
        .then((value) => done({ value }))
        .catch((error: unknown) => done({ error: String(error) }));
    }, command, args);
  } catch (error) {
    return { error: String(error) };
  }
}

describe("Tauri IPC capabilities", () => {
  it("allows generated app commands and denies sensitive plugin reads", async () => {
    const appInfo = await rawInvoke<{ appName: string; appVersion: string }>(
      "get_app_info",
    );
    expect(appInfo.error).toBeUndefined();
    expect(appInfo.value?.appName).toBe("Grayslate");

    const clipboardRead = await rawInvoke<string>(
      "plugin:clipboard-manager|read_text",
    );
    const hostnameRead = await rawInvoke<string>("plugin:os|hostname");

    expect(clipboardRead.error).toContain("not allowed");
    expect(hostnameRead.error).toContain("not allowed");
  });
});

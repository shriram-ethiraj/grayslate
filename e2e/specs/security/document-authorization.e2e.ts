import fs from "node:fs";
import path from "node:path";
import { browser, expect } from "@wdio/globals";
import { sandboxRoot } from "../../helpers/sandbox.js";

interface TauriInternals {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

interface InvocationError {
  kind: string | null;
  message: string;
}

async function invokeFromWebview(
  command: string,
  args: Record<string, unknown>,
): Promise<InvocationError> {
  return browser.executeAsync((name, payload, done) => {
    const internals = (window as unknown as { __TAURI_INTERNALS__: TauriInternals })
      .__TAURI_INTERNALS__;
    internals
      .invoke(name, payload)
      .then(() => done({ kind: null, message: "unexpectedly allowed" }))
      .catch((error: unknown) => {
        if (typeof error === "object" && error !== null) {
          const structured = error as Record<string, unknown>;
          done({
            kind: typeof structured.kind === "string" ? structured.kind : null,
            message: typeof structured.message === "string"
              ? structured.message
              : String(error),
          });
          return;
        }
        done({ kind: null, message: String(error) });
      });
  }, command, args);
}

describe("Rust-owned document authorization", () => {
  it("rejects forged file and autosave grants without changing disk contents", async () => {
    const victim = path.join(sandboxRoot, "forged-write-victim.txt");
    fs.writeFileSync(victim, "original", "utf8");

    const readError = await invokeFromWebview("read_file_content", {
      documentId: "00000000-0000-0000-0000-000000000000",
      documentGeneration: 1,
      requestId: 99,
      path: victim,
    });
    const writeError = await invokeFromWebview("write_file_content", {
      documentId: "00000000-0000-0000-0000-000000000000",
      documentGeneration: 1,
      content: "attacker-controlled",
      path: victim,
    });
    const deleteError = await invokeFromWebview("delete_file", {
      documentId: "00000000-0000-0000-0000-000000000000",
      documentGeneration: 1,
      path: victim,
      source: "slates",
    });
    const autosaveError = await invokeFromWebview("autosave_activate_document", {
      documentId: "00000000-0000-0000-0000-000000000000",
      documentGeneration: 1,
      languageHint: "text",
      path: victim,
      source: "slates",
    });

    expect(readError.kind).toBe("error");
    expect(readError.message).toContain("authorization");
    expect(writeError.message).toContain("authorization");
    expect(deleteError.message).toContain("authorization");
    expect(autosaveError.message).toContain("authorization");
    expect(fs.readFileSync(victim, "utf8")).toBe("original");
  });
});

import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { expectRejection } from "../../driver/invoke.js";
import { scenario } from "../../coverage/scenario.js";
import { sandboxRoot } from "../../helpers/sandbox.js";

/** A grant id that was never issued, standing in for an attacker's guess. */
const FORGED_ID = "00000000-0000-0000-0000-000000000000";
const FORGED_GRANT = { documentId: FORGED_ID, documentGeneration: 1 };

describe("Rust-owned document authorization", () => {
  scenario(
    "security.forged-grants",
    "rejects forged file and autosave grants without changing disk contents",
    async () => {
      const victim = path.join(sandboxRoot, "forged-write-victim.txt");
      fs.writeFileSync(victim, "original", "utf8");

      // expectRejection fails loudly if a command *succeeds* or if the driver
      // broke, so "authorization" can only be the backend refusing. The
      // previous version resolved an unexpected success into a message string
      // and relied on that string not containing the word.
      const attempts: [string, Record<string, unknown>][] = [
        ["read_file_content", { ...FORGED_GRANT, requestId: 99, path: victim }],
        ["write_file_content", { ...FORGED_GRANT, content: "attacker-controlled", path: victim }],
        ["delete_file", { ...FORGED_GRANT, path: victim, source: "slates" }],
        [
          "autosave_activate_document",
          { ...FORGED_GRANT, languageHint: "text", path: victim, source: "slates" },
        ],
      ];

      for (const [command, args] of attempts) {
        const message = await expectRejection(command, args);
        expect(message).toContain("authorization");
      }

      // The point of the test: refusing must also mean not writing.
      expect(fs.readFileSync(victim, "utf8")).toBe("original");
    },
  );
});

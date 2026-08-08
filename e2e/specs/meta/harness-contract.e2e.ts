import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { scenario } from "../../coverage/scenario.js";
import {
  readHasDeterministicMotionStyle,
  readIntlEnvironment,
  readPendingWork,
} from "../../driver/probe.js";
import { discoverSpecs } from "../../config/specs.js";
import { notesRoot, homeDirectory } from "../../helpers/sandbox.js";

/**
 * Guards on the harness itself.
 *
 * The suite's isolation used to rest on one line of config with no assertion
 * behind it: raising `maxInstances`, adding `specFileRetries`, or nesting the
 * spec array would silently let two spec files share a sandbox, and the only
 * symptom would be inventory assertions failing far away for reasons that look
 * like product bugs. These tests make that breakage loud and local.
 */
describe("Meta — harness contract", () => {
  scenario("harness.spec-discovery", "discovers every spec file on disk", async () => {
    const discovered = discoverSpecs();
    const onDisk: string[] = [];

    const walk = (directory: string): void => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const absolute = path.join(directory, entry.name);
        if (entry.isDirectory()) walk(absolute);
        else if (entry.isFile() && entry.name.endsWith(".e2e.ts")) onDisk.push(absolute);
      }
    };
    walk(path.resolve(process.cwd(), "e2e", "specs"));

    // A spec file that exists but never runs is worse than no spec at all: it
    // reads as coverage in review and provides none.
    expect([...discovered].sort()).toEqual([...onDisk].sort());
  });

  scenario(
    "harness.security-independent",
    "keeps security in a blocking CI job independent from functional and visual",
    async () => {
    const workflow = fs.readFileSync(
      path.resolve(process.cwd(), ".github", "workflows", "e2e.yml"),
      "utf8",
    );
    expect(workflow).toMatch(/^  functional:\s*$/m);
    expect(workflow).toMatch(/^  security:\s*$/m);
    expect(workflow).toMatch(/^  visual:\s*$/m);
    expect(workflow).toMatch(/^    continue-on-error: true\s*$/m);
    expect(workflow).not.toMatch(/security:\s*\n(?:.*\n)*?\s+needs:\s+functional/m);
    },
  );

  scenario(
    "harness.sandbox-isolation",
    "starts each spec file with a sandbox nobody else has written to",
    async () => {
    // Every spec seeds its own data. If this fails, the per-spec-file sandbox
    // wipe is no longer happening and cross-file contamination is possible.
    const stale = fs.existsSync(notesRoot)
      ? fs.readdirSync(notesRoot).filter((name) => !name.startsWith("."))
      : [];
    expect(stale).toEqual([]);
    },
  );

  scenario(
    "harness.isolated-home",
    "runs against an isolated HOME rather than the developer's",
    async () => {
    expect(process.env.HOME).toBe(homeDirectory);
    expect(homeDirectory).toContain(".e2e-tmp");
    },
  );

  scenario(
    "harness.deterministic-runtime",
    "installs deterministic motion and IPC tracking with a pinned locale and timezone",
    async () => {
      expect(await readHasDeterministicMotionStyle()).toBe(true);
      const pending = await readPendingWork();
      expect(pending).not.toBeNull();
      expect(pending?.phase).toBe("ready");
      expect(pending?.inFlight).toBe(0);
      expect(pending?.commands).toEqual([]);
      expect(pending?.tasks).toEqual([]);

      const intl = await readIntlEnvironment();
      expect(intl.locale).toBe("en-US");
      expect(intl.timeZone).toBe("UTC");
      expect(process.env.LC_ALL).toBe("en_US.UTF-8");
      expect(process.env.LANG).toBe("en_US.UTF-8");
      expect(process.env.TZ).toBe("UTC");
    },
  );
});

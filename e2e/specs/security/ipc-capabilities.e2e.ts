import { expect } from "@wdio/globals";
import { expectRejection, invokeInApp } from "../../driver/invoke.js";
import { scenario } from "../../coverage/scenario.js";

describe("Tauri IPC capabilities", () => {
  scenario(
    "security.capabilities",
    "allows generated app commands and denies sensitive plugin reads",
    async () => {
      const appInfo = await invokeInApp<{ appName: string; appVersion: string }>(
        "get_app_info",
      );
      expect(appInfo.appName).toBe("Grayslate");

      // expectRejection throws if the driver failed or the command succeeded,
      // so "not allowed" can only come from the capability system. The previous
      // version funnelled driver errors into the same field it asserted on,
      // which meant a broken session could satisfy a security assertion.
      expect(await expectRejection("plugin:clipboard-manager|read_text")).toContain(
        "not allowed",
      );
      expect(await expectRejection("plugin:os|hostname")).toContain("not allowed");
    },
  );

  scenario(
    "security.e2e-shims-present-in-test-build",
    "exposes the e2e-only commands solely because this build opted into them",
    async () => {
      // This build sets --features e2e, so the shim must be reachable. What
      // matters here is that the ACL admits the command at all — the grant then
      // fails on its own merits for a path outside any known root, which is
      // itself the authorization layer doing its job.
      const outcome = await expectRejection("e2e_save_path", {
        path: "/definitely/not/a/real/root/capability-probe.txt",
      });
      expect(outcome).not.toContain("not allowed by ACL");

      // The release guarantee itself is a build-configuration property: the
      // commands are behind `#[cfg(feature = "e2e")]` in src-tauri/src/lib.rs
      // and their ACL is generated only when CARGO_FEATURE_E2E is set. A
      // packaged release binary is verified by scripts/verify-release-build.mjs
      // rather than from inside a build that deliberately enables them.
      const unknownCommand = await expectRejection("e2e_definitely_not_a_command");
      expect(unknownCommand.length).toBeGreaterThan(0);
    },
  );
});

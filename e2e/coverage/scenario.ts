import { requirementById } from "./requirements.js";
import { waitForAppStable } from "../driver/wait.js";

export interface ScenarioOptions {
  /**
   * Ordinary scenarios must leave a stable, usable app behind. Use
   * `window-closed` only when successful completion intentionally destroys the
   * sole WebDriver window and the body verifies its terminal state out of
   * process.
   */
  completion?: "stable" | "window-closed";
}

const WINDOW_CLOSING_REQUIREMENTS = new Set([
  "file.slate.close-flushes",
  "file.identity.local-lifecycle",
]);

/**
 * Declare a test that claims one behavior from the registry.
 *
 * `scenario("csv.rows.move", "moves a row down", async () => { ... })`
 *
 * Two things follow from routing every test through here:
 *
 * 1. Coverage becomes checkable without running anything —
 *    `e2e/scripts/audit-coverage.mjs` parses these calls from the source and
 *    compares them to `requirements.ts`. Adding a behavior without a test, or
 *    deleting the test for one, fails CI.
 * 2. A scenario can be run on its own by id, which is how the "no test depends
 *    on a sibling" rule is actually verified rather than merely asserted.
 *
 * Unknown ids fail loudly at load time; a typo must not silently become
 * uncounted coverage.
 */
export function scenario(
  requirementId: string,
  title: string,
  body: () => Promise<void>,
  options: ScenarioOptions = {},
): void {
  const requirement = requirementById(requirementId);
  if (!requirement) {
    throw new Error(
      `scenario('${requirementId}') does not match any entry in e2e/coverage/requirements.ts. ` +
        "Add the behavior there first, or fix the id.",
    );
  }
  if (requirement.coverage !== "e2e") {
    throw new Error(
      `Requirement '${requirementId}' is marked coverage: '${requirement.coverage}', ` +
        "so it must not be claimed by a packaged scenario. Change the registry or the test.",
    );
  }

  const completion = options.completion ?? "stable";
  if (
    completion === "window-closed" &&
    !WINDOW_CLOSING_REQUIREMENTS.has(requirementId)
  ) {
    throw new Error(
      `Scenario '${requirementId}' cannot bypass stable cleanup. ` +
        "Only audited scenarios that intentionally destroy the sole app window may use window-closed.",
    );
  }
  if (
    completion === "stable" &&
    WINDOW_CLOSING_REQUIREMENTS.has(requirementId)
  ) {
    throw new Error(
      `Scenario '${requirementId}' destroys the app window and must declare ` +
        `{ completion: 'window-closed' }.`,
    );
  }

  it(`[${requirementId}] ${title}`, async () => {
    await waitForAppStable({
      message: `Scenario '${requirementId}' started before the application was stable.`,
    });

    await body();

    if (completion === "stable") {
      await waitForAppStable({
        message: `Scenario '${requirementId}' left application work unsettled.`,
      });
    }
  });
}

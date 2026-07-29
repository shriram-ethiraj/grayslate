import { requirementById } from "./requirements.js";

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

  it(`[${requirementId}] ${title}`, body);
}

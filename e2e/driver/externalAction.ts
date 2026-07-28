import { TIMEOUTS } from "../config/timeouts.js";
import { invokeInApp } from "./invoke.js";
import { waitFor } from "./wait.js";

export interface ExternalAction {
  kind: "reveal" | "open-url" | "open-path";
  target: string;
}

export async function queueExternalConfirmation(confirmed: boolean): Promise<void> {
  await invokeInApp<void>("e2e_queue_external_confirmation", { confirmed });
}

/**
 * Wait for the validated action handed to the native OS boundary.
 *
 * The test-only backend records only after production authorization,
 * destination parsing, confirmation, and revalidation have all succeeded.
 */
export async function waitForExternalAction(): Promise<ExternalAction> {
  let observed: ExternalAction | null = null;
  await waitFor(
    async () => {
      observed = await invokeInApp<ExternalAction | null>("e2e_take_external_action");
      return observed !== null;
    },
    {
      message: "No validated action reached the external OS boundary.",
      timeoutMs: TIMEOUTS.disk,
    },
  );
  if (!observed) throw new Error("External action wait completed without a result.");
  return observed;
}
